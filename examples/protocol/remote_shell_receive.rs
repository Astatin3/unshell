//! Remote shell controller example.
//!
//! This binary listens for the endpoint example, opens one remote shell session, sends a few
//! commands, and prints returned hook data until the shell closes.

use std::error::Error;
use std::net::TcpListener;

use unshell::leaves::remote_shell;
use unshell::leaves::remote_shell::OpenRequest;
use unshell::protocol::tree::encode_call_reply;
use unshell::protocol::tree::{
    ChildRoute, Endpoint, EndpointOutcome, Ingress, LocalEvent, ProtocolEndpoint,
};

fn main() -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind(remote_shell::endpoint::LISTEN_ADDR)?;
    println!("listening on {}", remote_shell::endpoint::LISTEN_ADDR);

    let (mut stream, peer_addr) = listener.accept()?;
    println!("accepted endpoint connection from {peer_addr}");

    let frame_rx = remote_shell::endpoint::spawn_frame_reader(stream.try_clone()?);
    let mut endpoint = ProtocolEndpoint::new(
        Vec::new(),
        None,
        vec![ChildRoute::registered(agent_path())],
        Vec::new(),
    );
    let hook_id = endpoint.allocate_hook_id();
    let shell_leaf_name = remote_shell::endpoint::RemoteShell::protocol_leaf_name();
    let open_procedure = remote_shell::endpoint::Open::protocol_procedure_id();

    remote_shell::endpoint::send_forward(
        &mut stream,
        endpoint.send_call(
            agent_path(),
            Some(shell_leaf_name),
            open_procedure.clone(),
            Some(hook_id),
            encode_call_reply(&OpenRequest).expect("remote shell open payload should encode"),
        )?,
    )?;

    for (index, command) in ["pwd\n", "whoami\n", "exit\n"].iter().enumerate() {
        remote_shell::endpoint::send_forward(
            &mut stream,
            endpoint.send_data(
                agent_path(),
                hook_id,
                open_procedure.clone(),
                command.as_bytes().to_vec(),
                index == 2,
            )?,
        )?;
    }

    for result in frame_rx {
        let frame = result?;
        let outcome = endpoint.receive(&Ingress::Child(agent_path()), frame)?;
        let EndpointOutcome::Local(event) = outcome else {
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

fn agent_path() -> Vec<String> {
    vec![String::from("agent")]
}
