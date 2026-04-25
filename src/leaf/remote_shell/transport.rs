use std::io::{self, ErrorKind, Read, Write};
use std::net::TcpStream;
use std::sync::mpsc::{self, Receiver};
use std::thread;

use unshell::protocol::FrameBytes;
use unshell::protocol::tree::EndpointOutcome;

pub const LISTEN_ADDR: &str = "127.0.0.1:4444";

#[allow(dead_code)]
pub fn send_forward(stream: &mut TcpStream, outcome: EndpointOutcome) -> io::Result<()> {
    write_frames(
        stream,
        &outcome
            .forward
            .into_iter()
            .map(|(_, frame)| frame)
            .collect::<Vec<_>>(),
    )
}

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
