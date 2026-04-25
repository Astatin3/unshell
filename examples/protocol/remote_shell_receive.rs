#[path = "../../src/leaf/remote_shell/mod.rs"]
mod remote_shell;

use std::error::Error;
use std::net::TcpListener;

use unshell::protocol::tree::{Endpoint, Ingress, LocalEvent};

fn main() -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind(remote_shell::LISTEN_ADDR)?;
    println!("listening on {}", remote_shell::LISTEN_ADDR);

    let (mut stream, peer_addr) = listener.accept()?;
    println!("accepted endpoint connection from {peer_addr}");

    let frame_rx = remote_shell::spawn_frame_reader(stream.try_clone()?);
    let mut endpoint = remote_shell::build_controller_endpoint();
    let hook_id = endpoint.allocate_hook_id();
    let shell_leaf_name = remote_shell::shell_leaf_name();
    let open_procedure = remote_shell::shell_open_procedure();

    remote_shell::send_forward(
        &mut stream,
        endpoint.send_call(
            remote_shell::agent_path(),
            Some(shell_leaf_name),
            open_procedure.clone(),
            Some(hook_id),
            remote_shell::shell_open_payload(),
        )?,
    )?;

    for (index, command) in ["pwd\n", "whoami\n", "exit\n"].iter().enumerate() {
        remote_shell::send_forward(
            &mut stream,
            endpoint.send_data(
                remote_shell::agent_path(),
                hook_id,
                open_procedure.clone(),
                command.as_bytes().to_vec(),
                index == 2,
            )?,
        )?;
    }

    for result in frame_rx {
        let frame = result?;
        let outcome = endpoint.receive(&Ingress::Child(remote_shell::agent_path()), frame)?;
        let Some(event) = outcome.event else {
            continue;
        };

        match event {
            LocalEvent::Data { message, .. } => {
                print!("{}", String::from_utf8_lossy(&message.data));

                if message.end_hook {
                    break;
                }
            }
            LocalEvent::Fault { message, .. } => {
                eprintln!("received protocol fault: 0x{:02X}", message.fault.0);
                break;
            }
            LocalEvent::Call { .. } => {}
        }
    }

    Ok(())
}
