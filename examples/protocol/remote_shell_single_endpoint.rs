//! Smallest in-process `RemoteShellLeaf` endpoint example.
//!
//! This example hosts exactly one protocol endpoint with exactly one leaf, `RemoteShellLeaf`, and
//! performs a local introspection request against that leaf. It does not open any sockets or spawn
//! a shell process, so it is the easiest place to see how the endpoint and leaf metadata fit
//! together.

use std::error::Error;

use unshell::leaves::remote_shell;
use unshell::protocol::tree::{EndpointOutcome, LocalEvent, ProtocolEndpoint};
use unshell::protocol::{INTROSPECTION_PROCEDURE_ID, LeafIntrospection};

fn main() -> Result<(), Box<dyn Error>> {
    let mut endpoint = ProtocolEndpoint::new(
        agent_path(),
        Some(Vec::new()),
        Vec::new(),
        vec![remote_shell::endpoint::RemoteShellEndpoint::protocol_leaf_spec()],
    );

    let hook_id = endpoint.allocate_hook_id();
    let outcome = endpoint.send_call(
        agent_path(),
        Some(remote_shell::endpoint::RemoteShellEndpoint::protocol_leaf_name()),
        INTROSPECTION_PROCEDURE_ID,
        Some(hook_id),
        Vec::new(),
    )?;

    let EndpointOutcome::Local(LocalEvent::Data { message, .. }) = outcome else {
        return Err("expected one local introspection response".into());
    };

    let payload = unshell::protocol::tree::decode_call_input::<LeafIntrospection>(&message.data)?;
    println!(
        "remote-shell examples normally listen on {}",
        remote_shell::endpoint::LISTEN_ADDR
    );
    println!("endpoint path: {:?}", agent_path());
    println!("leaf: {}", payload.leaf_name);
    println!("procedures: {:?}", payload.procedures);
    Ok(())
}

fn agent_path() -> Vec<String> {
    vec![String::from("agent")]
}
