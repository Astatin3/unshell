//! Remote shell endpoint example.
//!
//! This binary acts as the single remote-shell endpoint process. It connects to the controller
//! example over TCP, feeds inbound frames into the `ProcedureRuntime`, and flushes any resulting
//! protocol frames back to the controller.

use std::error::Error;
use std::net::TcpStream;
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

use unshell::leaves::remote_shell;
use unshell::protocol::tree::Ingress;

fn main() -> Result<(), Box<dyn Error>> {
    let mut stream = TcpStream::connect(remote_shell::endpoint::LISTEN_ADDR)?;
    let frame_rx = remote_shell::endpoint::spawn_frame_reader(stream.try_clone()?);
    let mut runtime = remote_shell::endpoint::build_agent_runtime();

    println!(
        "connected to controller at {}",
        remote_shell::endpoint::LISTEN_ADDR
    );

    loop {
        match frame_rx.recv_timeout(Duration::from_millis(25)) {
            Ok(result) => {
                let frame = result?;
                let outcome = runtime.receive(&Ingress::Parent, frame)?;
                remote_shell::endpoint::write_frames(&mut stream, &outcome.frames)?;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        let outcome = runtime.poll()?;
        remote_shell::endpoint::write_frames(&mut stream, &outcome.frames)?;
    }

    Ok(())
}
