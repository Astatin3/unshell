//! Small end-to-end example for the `leaf!` and `Procedure` macros.
//!
//! This stays entirely local. A controller endpoint opens one hook-backed procedure against a
//! single in-process leaf runtime, and the example decodes the returned reply payload.

use std::error::Error;
use std::{collections::BTreeMap, convert::Infallible, string::String};

use rkyv::{Archive, Deserialize, Serialize};
use unshell::protocol::tree::{
    Call, ChildRoute, EndpointOutcome, HookKey, Ingress, OutgoingData, Procedure, ProcedureEffect,
    ProcedureRuntime, ProcedureStore, ProtocolEndpoint,
};
use unshell::protocol::{PacketType, decode_frame};
use unshell::{Procedure, leaf};

#[derive(Default)]
struct EchoLeaf {
    sessions: BTreeMap<HookKey, EchoOpen>,
}

leaf! {
    id = "org.example.v1.echo",
    procedures = [EchoOpen],
    endpoint_struct = EchoLeaf,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct EchoRequest {
    text: String,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct EchoResponse {
    text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Procedure)]
#[procedure(leaf = EchoLeaf, name = "echo")]
struct EchoOpen {
    prefix: String,
    return_path: Vec<String>,
    hook_id: u64,
    sent_reply: bool,
}

impl ProcedureStore<EchoOpen> for EchoLeaf {
    fn procedure_sessions(&mut self) -> &mut BTreeMap<HookKey, EchoOpen> {
        &mut self.sessions
    }
}

impl Procedure<EchoLeaf> for EchoOpen {
    type Error = Infallible;
    type Input = EchoRequest;

    fn open(_leaf: &mut EchoLeaf, call: Call<Self::Input>) -> Result<Self, Self::Error> {
        let response_hook = call
            .response_hook
            .expect("example call declares a response hook");
        Ok(Self {
            prefix: call.input.text,
            return_path: response_hook.return_path,
            hook_id: response_hook.hook_id,
            sent_reply: false,
        })
    }

    fn poll(_leaf: &mut EchoLeaf, session: &mut Self) -> Result<ProcedureEffect, Self::Error> {
        if session.sent_reply {
            return Ok(ProcedureEffect::default());
        }
        session.sent_reply = true;
        Ok(ProcedureEffect::close(vec![OutgoingData {
            dst_path: session.return_path.clone(),
            hook_id: session.hook_id,
            procedure_id: EchoOpen::protocol_procedure_id(),
            data: unshell::protocol::tree::encode_call_reply(&EchoResponse {
                text: format!("echo: {}", session.prefix),
            })
            .expect("response should encode"),
            end_hook: true,
        }]))
    }
}

fn path(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_owned()).collect()
}

fn main() -> Result<(), Box<dyn Error>> {
    let endpoint = ProtocolEndpoint::new(
        path(&["agent"]),
        Some(Vec::new()),
        Vec::new(),
        vec![EchoLeaf::protocol_leaf_spec()],
    );
    let mut runtime = ProcedureRuntime::<EchoLeaf, EchoOpen>::new(endpoint, EchoLeaf::default());

    let mut controller = ProtocolEndpoint::new(
        Vec::new(),
        None,
        vec![ChildRoute {
            path: path(&["agent"]),
            registered: true,
        }],
        Vec::new(),
    );
    let hook_id = controller.allocate_hook_id();
    let controller_outcome = controller.send_call(
        path(&["agent"]),
        Some(EchoLeaf::protocol_leaf_name()),
        EchoOpen::protocol_procedure_id(),
        Some(hook_id),
        unshell::protocol::tree::encode_call_reply(&EchoRequest {
            text: String::from("hello leaf"),
        })?,
    )?;
    let EndpointOutcome::Forward { frame, .. } = controller_outcome else {
        return Err("expected controller to forward call".into());
    };

    let receive_outcome = runtime.receive(&Ingress::Parent, frame)?;
    assert!(receive_outcome.frames.is_empty());
    let outcome = runtime.poll()?;
    let [response_frame] = outcome.frames.as_slice() else {
        return Err("expected one response frame".into());
    };
    let parsed = decode_frame(response_frame.as_slice())?;
    assert_eq!(parsed.packet_type(), PacketType::Data);
    let response = unshell::protocol::tree::decode_call_input::<EchoResponse>(
        parsed.deserialize_data()?.data.as_slice(),
    )?;

    assert_eq!(EchoLeaf::protocol_leaf_name(), "org.example.v1.echo");
    assert_eq!(response.text, "echo: hello leaf");

    println!(
        "leaf={} procedure={} response={}",
        EchoLeaf::protocol_leaf_name(),
        EchoOpen::protocol_procedure_id(),
        response.text,
    );
    Ok(())
}
