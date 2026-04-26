use std::error::Error;
use std::{convert::Infallible, string::String};

use rkyv::{Archive, Deserialize, Serialize};
use unshell::protocol::tree::{
    Call, CallLeaf, ChildRoute, EndpointOutcome, Ingress, LeafRuntime, ProtocolEndpoint,
};
use unshell::protocol::{PacketType, decode_frame};
use unshell::{Leaf, procedures};

#[derive(Leaf)]
#[leaf(org = "org", product = "example", version = "v1", leaf_name = "echo")]
struct EchoLeaf {
    prefix: String,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct EchoRequest {
    text: String,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct EchoResponse {
    text: String,
}

#[procedures(error = Infallible)]
impl EchoLeaf {
    #[call]
    fn echo(&mut self, request: Call<EchoRequest>) -> EchoResponse {
        EchoResponse {
            text: format!("{}{}", self.prefix, request.input.text),
        }
    }
}

impl CallLeaf for EchoLeaf {
    type Error = Infallible;
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
    let mut runtime = LeafRuntime::new(
        endpoint,
        EchoLeaf {
            prefix: String::from("echo: "),
        },
    );

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
        EchoLeaf::protocol_procedure_id("echo").expect("known procedure suffix"),
        Some(hook_id),
        unshell::protocol::tree::encode_call_reply(&EchoRequest {
            text: String::from("hello leaf"),
        })?,
    )?;
    let EndpointOutcome::Forward { frame, .. } = controller_outcome else {
        return Err("expected controller to forward call".into());
    };

    let outcome = runtime.receive(&Ingress::Parent, frame)?;
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
        EchoLeaf::protocol_procedure_id("echo").expect("known procedure suffix"),
        response.text,
    );
    Ok(())
}
