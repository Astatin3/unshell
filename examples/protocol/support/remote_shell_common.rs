use std::io::{self, ErrorKind, Read, Write};
use std::net::TcpStream;
use std::sync::mpsc::{self, Receiver};
use std::thread;

use unshell::Leaf;
use unshell::protocol::FrameBytes;
use unshell::protocol::tree::{ChildRoute, EndpointOutcome, LocalEvent, ProtocolEndpoint};

pub const LISTEN_ADDR: &str = "127.0.0.1:4444";

#[derive(Leaf)]
#[leaf(procedures(start))]
pub struct RemoteShellLeaf;

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
pub fn build_agent_endpoint() -> ProtocolEndpoint {
    ProtocolEndpoint::new(
        agent_path(),
        Some(Vec::new()),
        Vec::new(),
        vec![RemoteShellLeaf::protocol_leaf_spec()],
    )
}

pub fn shell_leaf_name() -> String {
    RemoteShellLeaf::protocol_leaf_name()
}

pub fn shell_start_procedure() -> String {
    RemoteShellLeaf::protocol_procedure_id("start")
        .expect("remote shell leaf declares a start procedure")
}

pub fn write_frame(stream: &mut TcpStream, frame: &[u8]) -> io::Result<()> {
    let frame_len = u32::try_from(frame.len())
        .map_err(|_| io::Error::new(ErrorKind::InvalidData, "frame exceeds u32 transport size"))?;
    stream.write_all(&frame_len.to_be_bytes())?;
    stream.write_all(frame)?;
    stream.flush()?;
    Ok(())
}

pub fn pump_outcome(
    stream: &mut TcpStream,
    outcome: EndpointOutcome,
) -> io::Result<Option<LocalEvent>> {
    if let Some((_route, frame)) = outcome.forward {
        // These examples model one direct parent-child link over one TCP stream, so
        // any forwarded protocol frame is emitted on the same socket.
        write_frame(stream, &frame)?;
    }

    Ok(outcome.event)
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
