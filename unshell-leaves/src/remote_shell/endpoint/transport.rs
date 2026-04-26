use std::io::{self, ErrorKind, Read, Write};
use std::net::TcpStream;
use std::sync::mpsc::{self, Receiver};
use std::thread;

use unshell::protocol::FrameBytes;
use unshell::protocol::tree::EndpointOutcome;

/// TCP listen address used by the remote shell examples.
pub const LISTEN_ADDR: &str = "127.0.0.1:4444";
const MAX_FRAME_BYTES: usize = 1024 * 1024;

/// Writes the forwarded frame produced by one endpoint outcome.
pub fn send_forward(stream: &mut TcpStream, outcome: EndpointOutcome) -> io::Result<()> {
    match outcome {
        EndpointOutcome::Forward { frame, .. } => write_frames(stream, &[frame]),
        EndpointOutcome::Local(_) | EndpointOutcome::Dropped => write_frames(stream, &[]),
    }
}

/// Writes one or more framed packets onto the example TCP stream.
pub fn write_frames(stream: &mut TcpStream, frames: &[FrameBytes]) -> io::Result<()> {
    for frame in frames {
        let frame_len = u32::try_from(frame.len()).map_err(|_| {
            io::Error::new(ErrorKind::InvalidData, "frame exceeds u32 transport size")
        })?;
        stream.write_all(&frame_len.to_be_bytes())?;
        stream.write_all(frame)?;
    }
    stream.flush()?;
    Ok(())
}

/// Spawns the example frame reader that lifts prefixed frames off the TCP stream.
pub fn spawn_frame_reader(mut stream: TcpStream) -> Receiver<io::Result<FrameBytes>> {
    let (tx, rx) = mpsc::sync_channel(64);

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
    let Some(len_bytes) = read_prefix(stream)? else {
        return Ok(None);
    };

    let frame_len = u32::from_be_bytes(len_bytes) as usize;
    if frame_len > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "frame exceeds remote shell example transport limit",
        ));
    }
    let mut bytes = vec![0u8; frame_len];
    stream.read_exact(&mut bytes)?;

    let mut frame = FrameBytes::with_capacity(bytes.len());
    frame.extend_from_slice(&bytes);
    Ok(Some(frame))
}

fn read_prefix(stream: &mut TcpStream) -> io::Result<Option<[u8; 4]>> {
    let mut len_bytes = [0u8; 4];
    let mut filled = 0usize;

    while filled < len_bytes.len() {
        match stream.read(&mut len_bytes[filled..]) {
            Ok(0) if filled == 0 => return Ok(None),
            Ok(0) => return Err(io::Error::from(ErrorKind::UnexpectedEof)),
            Ok(read_len) => filled += read_len,
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }

    Ok(Some(len_bytes))
}
