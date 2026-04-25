#[path = "support/remote_shell_common.rs"]
mod common;

use std::error::Error;
use std::net::TcpStream;
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

use unshell::protocol::tree::Ingress;

fn main() -> Result<(), Box<dyn Error>> {
    let mut stream = TcpStream::connect(common::LISTEN_ADDR)?;
    let frame_rx = common::spawn_frame_reader(stream.try_clone()?);
    let mut runtime = common::build_agent_runtime();

    println!("connected to controller at {}", common::LISTEN_ADDR);

    loop {
        match frame_rx.recv_timeout(Duration::from_millis(25)) {
            Ok(result) => {
                let frame = result?;
                let outcome = runtime.receive(&Ingress::Parent, frame)?;
                common::write_frames(&mut stream, &outcome.frames)?;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        let outcome = runtime.poll()?;
        common::write_frames(&mut stream, &outcome.frames)?;
    }

    Ok(())
}
