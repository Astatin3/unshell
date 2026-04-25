use std::error::Error;

use unshell::Leaf;
use unshell::protocol::tree::{Endpoint, Ingress, LocalEvent, ProtocolEndpoint};

#[derive(Leaf)]
#[leaf(org = "org", product = "example", version = "v1", leaf_name = "echo")]
#[leaf(procedures(call, stream))]
struct EchoLeaf;

fn path(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_owned()).collect()
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut endpoint = ProtocolEndpoint::new(
        path(&["agent"]),
        Some(Vec::new()),
        Vec::new(),
        vec![EchoLeaf::protocol_leaf_spec()],
    );

    let hook_id = endpoint.allocate_hook_id();
    let frame = endpoint.make_call(
        path(&["agent"]),
        Some(EchoLeaf::protocol_leaf_name()),
        EchoLeaf::protocol_procedure_id("call").expect("known procedure suffix"),
        Some(hook_id),
        b"hello leaf".to_vec(),
    )?;

    let outcome = endpoint.receive(&Ingress::Parent, frame)?;
    let Some(LocalEvent::Call { header, message }) = outcome.event else {
        return Err("expected local leaf call".into());
    };

    assert_eq!(header.dst_leaf.as_deref(), Some("org.example.v1.echo"));
    assert_eq!(message.procedure_id, "org.example.v1.echo.call");

    println!(
        "leaf={} procedure={}",
        EchoLeaf::protocol_leaf_name(),
        message.procedure_id
    );
    Ok(())
}
