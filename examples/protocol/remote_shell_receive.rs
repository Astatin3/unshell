#[path = "support/remote_shell_common.rs"]
mod common;

use std::error::Error;
use std::net::TcpListener;

use unshell::protocol::tree::{Endpoint, Ingress, LocalEvent};

fn main() -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind(common::LISTEN_ADDR)?;
    println!("listening on {}", common::LISTEN_ADDR);

    let (mut stream, peer_addr) = listener.accept()?;
    println!("accepted endpoint connection from {peer_addr}");

    let frame_rx = common::spawn_frame_reader(stream.try_clone()?);
    let mut endpoint = common::build_controller_endpoint();
    let hook_id = endpoint.allocate_hook_id();
    let shell_leaf_name = common::shell_leaf_name();
    let start_procedure = common::shell_start_procedure();

    let outcome = endpoint.send_call(
        common::agent_path(),
        Some(shell_leaf_name),
        start_procedure.clone(),
        Some(hook_id),
        Vec::new(),
    )?;
    let _ = common::pump_outcome(&mut stream, outcome)?;

    let mut commands_sent = false;

    for result in frame_rx {
        let frame = result?;
        let outcome = endpoint.receive(&Ingress::Child(common::agent_path()), frame)?;
        let event = common::pump_outcome(&mut stream, outcome)?;

        let Some(event) = event else {
            continue;
        };

        match event {
            LocalEvent::Data { message, .. } => {
                print!("{}", String::from_utf8_lossy(&message.data));

                if !commands_sent {
                    commands_sent = true;
                    for (index, command) in ["pwd\n", "whoami\n", "exit\n"].iter().enumerate() {
                        let outcome = endpoint.send_data(
                            common::agent_path(),
                            hook_id,
                            start_procedure.clone(),
                            command.as_bytes().to_vec(),
                            index == 2,
                        )?;
                        let _ = common::pump_outcome(&mut stream, outcome)?;
                    }
                }

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
