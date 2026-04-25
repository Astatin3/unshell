use std::io::{self, Read};
use std::process::{Child, ChildStdin, ExitStatus};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use unshell::protocol::tree::OutgoingData;

use super::errors::ShellLeafError;

pub(super) struct ShellSession {
    pub(super) child: Child,
    pub(super) stdin: Option<ChildStdin>,
    output_rx: Receiver<OutputEvent>,
    return_path: Vec<String>,
    hook_id: u64,
    procedure_id: String,
    pub(super) readers_closed: usize,
    pub(super) exit_status: Option<ExitStatus>,
    pub(super) local_end_sent: bool,
}

enum OutputEvent {
    Chunk(Vec<u8>),
    ReaderClosed,
}

impl ShellSession {
    pub(super) fn spawn(
        return_path: Vec<String>,
        hook_id: u64,
        procedure_id: String,
    ) -> Result<Self, ShellLeafError> {
        let mut command = if cfg!(windows) {
            let mut command = std::process::Command::new("cmd.exe");
            command.arg("/Q");
            command
        } else {
            let mut command = std::process::Command::new("/bin/sh");
            command.arg("-i");
            command
        };

        let mut child = command
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
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

    pub(super) fn packet(&self, data: Vec<u8>, end_hook: bool) -> OutgoingData {
        OutgoingData {
            dst_path: self.return_path.clone(),
            hook_id: self.hook_id,
            procedure_id: self.procedure_id.clone(),
            data,
            end_hook,
        }
    }

    pub(super) fn terminate(&mut self) -> Result<(), ShellLeafError> {
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

    pub(super) fn drain_output(&mut self, outgoing: &mut Vec<OutgoingData>) {
        loop {
            match self.output_rx.try_recv() {
                Ok(OutputEvent::Chunk(bytes)) => outgoing.push(self.packet(bytes, false)),
                Ok(OutputEvent::ReaderClosed) => self.readers_closed += 1,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.readers_closed = 2;
                    break;
                }
            }
        }
    }
}

pub(super) fn close_session(
    mut session: ShellSession,
) -> Result<Vec<OutgoingData>, ShellLeafError> {
    session.terminate()?;
    if session.local_end_sent {
        return Ok(Vec::new());
    }

    session.local_end_sent = true;
    Ok(vec![session.packet(Vec::new(), true)])
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
