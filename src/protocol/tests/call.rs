use alloc::{borrow::ToOwned, format, string::String, vec, vec::Vec};
use core::convert::Infallible;

use rkyv::{Archive, Deserialize, Serialize};

use crate::protocol::tree::{
    Call, CallLeaf, ChildRoute, ConnectionState, Ingress, LeafRuntime, ProtocolEndpoint,
    decode_call_input, encode_call_reply,
};
use crate::protocol::{PacketType, decode_frame};
use crate::{Leaf, procedures};

fn path(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_owned()).collect()
}

#[derive(Leaf)]
#[leaf(id = "org.example.v1.echo")]
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

#[test]
fn leaf_runtime_dispatches_generated_call_procedure() {
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
            state: ConnectionState::Registered,
        }],
        Vec::new(),
    );
    let hook_id = controller.allocate_hook_id();
    let controller_outcome = controller
        .send_call(
            path(&["agent"]),
            Some(EchoLeaf::protocol_leaf_name()),
            EchoLeaf::protocol_procedure_id("echo").expect("generated suffix should resolve"),
            Some(hook_id),
            encode_call_reply(&EchoRequest {
                text: String::from("hello"),
            })
            .expect("request should encode"),
        )
        .expect("call should encode");
    let Some((_, frame)) = controller_outcome.forward else {
        panic!("controller should forward call to child");
    };

    let outcome = runtime
        .receive(&Ingress::Parent, frame)
        .expect("runtime should handle call");
    let [response_frame] = outcome.frames.as_slice() else {
        panic!("expected one response frame");
    };

    let parsed = decode_frame(response_frame.as_slice()).expect("response frame should decode");
    assert_eq!(parsed.packet_type(), PacketType::Data);
    let response = decode_call_input::<EchoResponse>(
        parsed
            .deserialize_data()
            .expect("data payload should deserialize")
            .data
            .as_slice(),
    )
    .expect("typed response should decode");
    assert_eq!(response.text, "echo: hello");
}
