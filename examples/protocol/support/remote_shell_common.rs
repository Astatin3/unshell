use std::collections::BTreeMap;
use std::fmt;
use std::io::{self, ErrorKind, Read, Write};
use std::net::TcpStream;
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use unshell::protocol::FrameBytes;
use unshell::protocol::tree::{
    Call, CallLeaf, ChildRoute, HookKey, IncomingData, IncomingFault, LeafRuntime, OutgoingData,
    ProtocolEndpoint,
};
use unshell::{Leaf, procedures};

pub const LISTEN_ADDR: &str = "127.0.0.1:4444";

#[derive(Default, Leaf)]
pub struct RemoteShellLeaf {
    sessions: BTreeMap<HookKey, ShellSession>,
}

#[procedures(error = ShellLeafError)]
impl RemoteShellLeaf {
    #[call]
    fn open(&mut self, call: Call<()>) -> Result<(), ShellLeafError> {
        let hook_key = call.response_hook.ok_or(ShellLeafError::MissingHook)?;
        let session = ShellSession::spawn(
            hook_key.return_path.clone(),
            hook_key.hook_id,
            call.procedure_id,
        )?;

        if let Some(mut previous) = self.sessions.insert(hook_key, session) {
            previous.terminate()?;
        }
        Ok(())
    }
}

impl CallLeaf for RemoteShellLeaf {
    type Error = ShellLeafError;

    fn on_data(&mut self, data: IncomingData) -> Result<Vec<OutgoingData>, Self::Error> {
        let Some(session) = self.sessions.get_mut(&data.hook_key) else {
            return Ok(Vec::new());
        };

        if !data.message.data.is_empty() {
            let Some(stdin) = session.stdin.as_mut() else {
                return Ok(Vec::new());
            };
            stdin.write_all(&data.message.data)?;
            stdin.flush()?;
        }

        if !data.message.end_hook {
            return Ok(Vec::new());
        }

        let session = self
            .sessions
            .remove(&data.hook_key)
            .ok_or(ShellLeafError::MissingSession)?;
        close_session(session)
    }

    fn on_fault(&mut self, fault: IncomingFault) -> Result<(), Self::Error> {
        if let Some(mut session) = self.sessions.remove(&fault.hook_key) {
            session.terminate()?;
        }
        Ok(())
    }

    fn poll(&mut self) -> Result<Vec<OutgoingData>, Self::Error> {
        let mut outgoing = Vec::new();
        let mut closed = Vec::new();

        for key in self.sessions.keys().cloned().collect::<Vec<_>>() {
            let Some(session) = self.sessions.get_mut(&key) else {
                continue;
            };

            loop {
                match session.output_rx.try_recv() {
                    Ok(OutputEvent::Chunk(bytes)) => {
                        outgoing.push(session.packet(bytes, false));
                    }
                    Ok(OutputEvent::ReaderClosed) => {
                        session.readers_closed += 1;
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        session.readers_closed = 2;
                        break;
                    }
                }
            }

            if session.local_end_sent {
                continue;
            }

            if session.exit_status.is_none() {
                session.exit_status = session.child.try_wait()?;
            }

            if session.exit_status.is_some() && session.readers_closed >= 2 {
                outgoing.push(session.packet(Vec::new(), true));
                session.local_end_sent = true;
                closed.push(key);
            }
        }

        for key in closed {
            self.sessions.remove(&key);
        }

        Ok(outgoing)
    }
}

pub fn agent_path() -> Vec<String> {
    path(&["agent"])
}

pub fn path(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_owned()).collect()
}

#[allow(dead_code)]
pub fn build_controller_endpoint() -> ProtocolEndpoint {
    ProtocolEndpoint::new(
        Vec::new(),
        None,
        vec![ChildRoute::registered(agent_path())],
        Vec::new(),
    )
}

#[allow(dead_code)]
pub fn build_agent_runtime() -> LeafRuntime<RemoteShellLeaf> {
    let endpoint = ProtocolEndpoint::new(
        agent_path(),
        Some(Vec::new()),
        Vec::new(),
        vec![RemoteShellLeaf::protocol_leaf_spec()],
    );
    LeafRuntime::new(endpoint, RemoteShellLeaf::default())
}

#[allow(dead_code)]
pub fn shell_leaf_name() -> String {
    RemoteShellLeaf::protocol_leaf_name()
}

#[allow(dead_code)]
pub fn shell_open_procedure() -> String {
    RemoteShellLeaf::protocol_procedure_id("open")
        .expect("remote shell leaf declares an open procedure")
}

#[allow(dead_code)]
pub fn shell_open_payload() -> Vec<u8> {
    unshell::protocol::tree::encode_call_reply(&()).expect("unit shell open payload should encode")
}

pub fn write_frame(stream: &mut TcpStream, frame: &[u8]) -> io::Result<()> {
    let frame_len = u32::try_from(frame.len())
        .map_err(|_| io::Error::new(ErrorKind::InvalidData, "frame exceeds u32 transport size"))?;
    stream.write_all(&frame_len.to_be_bytes())?;
    stream.write_all(frame)?;
    stream.flush()?;
    Ok(())
}

pub fn write_frames(stream: &mut TcpStream, frames: &[FrameBytes]) -> io::Result<()> {
    for frame in frames {
        write_frame(stream, frame)?;
    }
    Ok(())
}

pub fn spawn_frame_reader(mut stream: TcpStream) -> Receiver<io::Result<FrameBytes>> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        loop {
            match read_frame(&mut stream) {
                Ok(Some(frame)) => {
                    if tx.send(Ok(frame)).is_err() {
                        break;
                    }
                }
                Ok(None) => break,
                Err(error) => {
                    let _ = tx.send(Err(error));
                    break;
                }
            }
        }
    });

    rx
}

fn close_session(mut session: ShellSession) -> Result<Vec<OutgoingData>, ShellLeafError> {
    session.terminate()?;
    if session.local_end_sent {
        return Ok(Vec::new());
    }

    session.local_end_sent = true;
    Ok(vec![session.packet(Vec::new(), true)])
}

fn read_frame(stream: &mut TcpStream) -> io::Result<Option<FrameBytes>> {
    let mut len_bytes = [0u8; 4];
    match stream.read_exact(&mut len_bytes) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(error),
    }

    let frame_len = u32::from_be_bytes(len_bytes) as usize;
    let mut bytes = vec![0u8; frame_len];
    match stream.read_exact(&mut bytes) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(error),
    }

    let mut frame = FrameBytes::with_capacity(bytes.len());
    frame.extend_from_slice(&bytes);
    Ok(Some(frame))
}

struct ShellSession {
    child: Child,
    stdin: Option<ChildStdin>,
    output_rx: Receiver<OutputEvent>,
    return_path: Vec<String>,
    hook_id: u64,
    procedure_id: String,
    readers_closed: usize,
    exit_status: Option<ExitStatus>,
    local_end_sent: bool,
}

enum OutputEvent {
    Chunk(Vec<u8>),
    ReaderClosed,
}

impl ShellSession {
    fn spawn(
        return_path: Vec<String>,
        hook_id: u64,
        procedure_id: String,
    ) -> Result<Self, ShellLeafError> {
        let mut command = if cfg!(windows) {
            let mut command = Command::new("cmd.exe");
            command.arg("/Q");
            command
        } else {
            let mut command = Command::new("/bin/sh");
            command.arg("-i");
            command
        };

        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("failed to capture shell stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("failed to capture shell stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| io::Error::other("failed to capture shell stderr"))?;

        let (tx, rx) = mpsc::channel();
        spawn_pipe_reader(stdout, tx.clone());
        spawn_pipe_reader(stderr, tx);

        Ok(Self {
            child,
            stdin: Some(stdin),
            output_rx: rx,
            return_path,
            hook_id,
            procedure_id,
            readers_closed: 0,
            exit_status: None,
            local_end_sent: false,
        })
    }

    fn packet(&self, data: Vec<u8>, end_hook: bool) -> OutgoingData {
        OutgoingData {
            dst_path: self.return_path.clone(),
            hook_id: self.hook_id,
            procedure_id: self.procedure_id.clone(),
            data,
            end_hook,
        }
    }

    fn terminate(&mut self) -> Result<(), ShellLeafError> {
        self.stdin.take();
        match self.child.try_wait()? {
            Some(status) => {
                self.exit_status = Some(status);
                Ok(())
            }
            None => {
                self.child.kill()?;
                self.exit_status = Some(self.child.wait()?);
                Ok(())
            }
        }
    }
}

fn spawn_pipe_reader<R>(mut reader: R, tx: mpsc::Sender<OutputEvent>)
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        loop {
            let mut buffer = [0u8; 1024];
            match reader.read(&mut buffer) {
                Ok(0) => {
                    let _ = tx.send(OutputEvent::ReaderClosed);
                    break;
                }
                Ok(read_len) => {
                    if tx
                        .send(OutputEvent::Chunk(buffer[..read_len].to_vec()))
                        .is_err()
                    {
                        break;
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => {
                    let _ = tx.send(OutputEvent::Chunk(
                        format!("shell pipe read error: {error}\n").into_bytes(),
                    ));
                    let _ = tx.send(OutputEvent::ReaderClosed);
                    break;
                }
            }
        }
    });
}

#[derive(Debug)]
pub enum ShellLeafError {
    Io(io::Error),
    MissingHook,
    MissingSession,
}

impl fmt::Display for ShellLeafError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::MissingHook => f.write_str("shell open requires a response hook"),
            Self::MissingSession => f.write_str("shell session missing for active hook"),
        }
    }
}

impl std::error::Error for ShellLeafError {}

impl From<io::Error> for ShellLeafError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}
