//! Crossbeam-channel router leaf example.
//!
//! This example wires a root controller to an `agent` node, promotes a staged
//! child connection on that agent via the `add_connection` procedure, and then
//! queries the grandchild's connection snapshot through a fully routed call/reply
//! exchange.

use std::error::Error;

use crossbeam_channel::{Receiver, Sender, unbounded};
use unshell::leaves::crossbeam_channel::{
    ConnectionRequest, ConnectionSnapshot, CrossbeamChannelLeaf, CrossbeamEnvelope,
};
use unshell::protocol::tree::ProtocolEndpoint;
use unshell::protocol::tree::{
    ChildRoute, Endpoint, EndpointOutcome, Ingress, LeafRuntime, decode_call_input,
    encode_call_reply,
};

fn main() -> Result<(), Box<dyn Error>> {
    let (mut agent, root_to_agent) = ChannelNode::new(path(&["agent"]));
    let (mut child, agent_to_child) = ChannelNode::new(path(&["agent", "child"]));
    let (agent_to_root, root_rx) = unbounded();

    let mut root = ProtocolEndpoint::new(
        Vec::new(),
        None,
        vec![ChildRoute::registered(path(&["agent"]))],
        Vec::new(),
    );

    agent.stage_connection(Vec::new(), agent_to_root);
    agent.connect_staged(Vec::new())?;

    child.stage_connection(path(&["agent"]), root_to_agent.clone());
    child.connect_staged(path(&["agent"]))?;

    agent.stage_connection(path(&["agent", "child"]), agent_to_child);

    call_root(
        &mut root,
        &root_to_agent,
        &mut agent,
        &mut child,
        &root_rx,
        path(&["agent"]),
        CrossbeamChannelLeaf::protocol_procedure_id("add_connection").expect("procedure exists"),
        encode_call_reply(&ConnectionRequest {
            peer_path: path(&["agent", "child"]),
        })?,
    )?;

    let reply = call_root(
        &mut root,
        &root_to_agent,
        &mut agent,
        &mut child,
        &root_rx,
        path(&["agent", "child"]),
        CrossbeamChannelLeaf::protocol_procedure_id("get_connections").expect("procedure exists"),
        encode_call_reply(&())?,
    )?;
    let snapshot = decode_call_input::<ConnectionSnapshot>(reply.as_slice())?;

    println!("child parent: {:?}", snapshot.parent);
    println!("child children: {:?}", snapshot.children);

    Ok(())
}

struct ChannelNode {
    runtime: LeafRuntime<CrossbeamChannelLeaf>,
    rx: Receiver<CrossbeamEnvelope>,
}

impl ChannelNode {
    fn new(path: Vec<String>) -> (Self, Sender<CrossbeamEnvelope>) {
        let (tx, rx) = unbounded();
        let endpoint = ProtocolEndpoint::new(
            path,
            None,
            Vec::new(),
            vec![CrossbeamChannelLeaf::protocol_leaf_spec()],
        );
        (
            Self {
                runtime: LeafRuntime::new(endpoint, CrossbeamChannelLeaf::default()),
                rx,
            },
            tx,
        )
    }

    fn stage_connection(&mut self, peer_path: Vec<String>, sender: Sender<CrossbeamEnvelope>) {
        let _ = self.runtime.leaf_mut().stage_connection(peer_path, sender);
    }

    fn connect_staged(&mut self, peer_path: Vec<String>) -> Result<(), Box<dyn Error>> {
        let runtime = &mut self.runtime;
        let mut leaf = core::mem::take(runtime.leaf_mut());
        let result = leaf.connect_staged(runtime.endpoint_mut(), peer_path);
        *runtime.leaf_mut() = leaf;
        result?;
        Ok(())
    }

    fn drain(&mut self) -> Result<usize, Box<dyn Error>> {
        let mut processed = 0usize;
        while let Ok(envelope) = self.rx.try_recv() {
            let outcome = self
                .runtime
                .receive_routed(&envelope.ingress, envelope.frame)?;
            self.runtime.route_forwarded(outcome.forwarded)?;
            processed += 1;
        }
        Ok(processed)
    }
}

fn call_root(
    root: &mut ProtocolEndpoint,
    root_to_agent: &Sender<CrossbeamEnvelope>,
    agent: &mut ChannelNode,
    child: &mut ChannelNode,
    root_rx: &Receiver<CrossbeamEnvelope>,
    dst_path: Vec<String>,
    procedure_id: String,
    data: Vec<u8>,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let hook_id = root.allocate_hook_id();
    let outcome = root.send_call(
        dst_path,
        Some(CrossbeamChannelLeaf::protocol_leaf_name()),
        procedure_id,
        Some(hook_id),
        data,
    )?;
    let EndpointOutcome::Forward { frame, .. } = outcome else {
        return Err("root call did not forward".into());
    };
    root_to_agent.send(CrossbeamEnvelope {
        ingress: Ingress::Parent,
        frame,
    })?;

    for _ in 0..16 {
        let mut progress = 0usize;
        progress += agent.drain()?;
        progress += child.drain()?;

        while let Ok(envelope) = root_rx.try_recv() {
            progress += 1;
            let outcome = root.receive(&envelope.ingress, envelope.frame)?;
            if let EndpointOutcome::Local(event) = outcome {
                match event {
                    unshell::protocol::tree::LocalEvent::Data { message, .. } => {
                        return Ok(message.data);
                    }
                    unshell::protocol::tree::LocalEvent::Fault { message, .. } => {
                        return Err(format!("routed call faulted: {:?}", message.fault).into());
                    }
                    unshell::protocol::tree::LocalEvent::Call { .. } => {}
                }
            }
        }

        if progress == 0 {
            break;
        }
    }

    Err("timed out waiting for routed reply".into())
}

fn path(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_owned()).collect()
}
