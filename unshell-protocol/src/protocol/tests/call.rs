use alloc::{borrow::ToOwned, format, string::String, vec, vec::Vec};
use core::convert::Infallible;

use rkyv::{Archive, Deserialize, Serialize};

use crate::protocol::tree::{
    Call, CallLeaf, ChildRoute, EndpointOutcome, Ingress, LeafRuntime, ProtocolEndpoint,
    decode_call_input, encode_call_reply,
};
use crate::protocol::{PacketType, decode_frame};
use crate::{leaf, procedures};

fn path(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_owned()).collect()
}

struct EchoLeaf {
    prefix: String,
}

#[leaf(id = "org.example.v1.echo", endpoint_struct = EchoLeaf, procedures = ["echo"])]
struct Echo;

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
            registered: true,
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
    let EndpointOutcome::Forward { frame, .. } = controller_outcome else {
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

#[derive(Default)]
struct TopologyLeaf;

#[leaf(
    id = "org.example.v1.topology",
    endpoint_struct = TopologyLeaf,
    procedures = ["add_child", "remove_child", "connections"]
)]
struct Topology;

#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct ChildRequest {
    child_path: Vec<String>,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct ConnectionsReply {
    parent: Option<Vec<String>>,
    children: Vec<Vec<String>>,
}

#[procedures(error = Infallible)]
impl TopologyLeaf {
    #[call]
    fn add_child(
        &mut self,
        endpoint: &mut ProtocolEndpoint,
        request: ChildRequest,
    ) -> ConnectionsReply {
        endpoint
            .upsert_child_route(ChildRoute::registered(request.child_path))
            .expect("topology mutation should satisfy direct-child invariants");
        ConnectionsReply {
            parent: endpoint.parent_path().map(<[String]>::to_vec),
            children: endpoint
                .child_routes()
                .iter()
                .map(|child| child.path.clone())
                .collect(),
        }
    }

    #[call]
    fn remove_child(
        &mut self,
        endpoint: &mut ProtocolEndpoint,
        request: ChildRequest,
    ) -> ConnectionsReply {
        endpoint.remove_child_route(&request.child_path);
        ConnectionsReply {
            parent: endpoint.parent_path().map(<[String]>::to_vec),
            children: endpoint
                .child_routes()
                .iter()
                .map(|child| child.path.clone())
                .collect(),
        }
    }

    #[call]
    fn connections(&mut self, endpoint: &ProtocolEndpoint) -> ConnectionsReply {
        ConnectionsReply {
            parent: endpoint.parent_path().map(<[String]>::to_vec),
            children: endpoint
                .child_routes()
                .iter()
                .map(|child| child.path.clone())
                .collect(),
        }
    }
}

impl CallLeaf for TopologyLeaf {
    type Error = Infallible;
}

#[test]
fn generated_call_procedure_can_query_and_mutate_endpoint_topology() {
    let endpoint = ProtocolEndpoint::new(
        path(&["agent"]),
        Some(Vec::new()),
        Vec::new(),
        vec![TopologyLeaf::protocol_leaf_spec()],
    );
    let mut runtime = LeafRuntime::new(endpoint, TopologyLeaf);

    let mut controller = ProtocolEndpoint::new(
        Vec::new(),
        None,
        vec![ChildRoute::registered(path(&["agent"]))],
        Vec::new(),
    );

    let add_hook = controller.allocate_hook_id();
    let add_child = controller
        .send_call(
            path(&["agent"]),
            Some(TopologyLeaf::protocol_leaf_name()),
            TopologyLeaf::protocol_procedure_id("add_child").expect("suffix should resolve"),
            Some(add_hook),
            encode_call_reply(&ChildRequest {
                child_path: path(&["agent", "child"]),
            })
            .expect("request should encode"),
        )
        .expect("call should encode");
    let EndpointOutcome::Forward {
        frame: add_child_frame,
        ..
    } = add_child
    else {
        panic!("controller should forward add-child call");
    };
    let add_outcome = runtime
        .receive(&Ingress::Parent, add_child_frame)
        .expect("runtime should mutate topology");
    let [response] = add_outcome.frames.as_slice() else {
        panic!("expected add-child response frame");
    };
    let parsed = decode_frame(response).expect("response should decode");
    let reply = decode_call_input::<ConnectionsReply>(
        parsed
            .deserialize_data()
            .expect("reply data should decode")
            .data
            .as_slice(),
    )
    .expect("typed reply should decode");
    assert_eq!(reply.parent, Some(Vec::new()));
    assert_eq!(reply.children, vec![path(&["agent", "child"])]);
    assert_eq!(runtime.endpoint().child_routes().len(), 1);

    let list_hook = controller.allocate_hook_id();
    let list = controller
        .send_call(
            path(&["agent"]),
            Some(TopologyLeaf::protocol_leaf_name()),
            TopologyLeaf::protocol_procedure_id("connections").expect("suffix should resolve"),
            Some(list_hook),
            encode_call_reply(&()).expect("unit request should encode"),
        )
        .expect("list call should encode");
    let EndpointOutcome::Forward {
        frame: list_frame, ..
    } = list
    else {
        panic!("controller should forward connections call");
    };
    let list_outcome = runtime
        .receive(&Ingress::Parent, list_frame)
        .expect("runtime should return topology snapshot");
    let [list_response] = list_outcome.frames.as_slice() else {
        panic!("expected connections response frame");
    };
    let list_reply = decode_call_input::<ConnectionsReply>(
        decode_frame(list_response)
            .expect("response should decode")
            .deserialize_data()
            .expect("data should deserialize")
            .data
            .as_slice(),
    )
    .expect("typed reply should decode");
    assert_eq!(list_reply.children, vec![path(&["agent", "child"])]);

    let remove_hook = controller.allocate_hook_id();
    let remove = controller
        .send_call(
            path(&["agent"]),
            Some(TopologyLeaf::protocol_leaf_name()),
            TopologyLeaf::protocol_procedure_id("remove_child").expect("suffix should resolve"),
            Some(remove_hook),
            encode_call_reply(&ChildRequest {
                child_path: path(&["agent", "child"]),
            })
            .expect("request should encode"),
        )
        .expect("remove call should encode");
    let EndpointOutcome::Forward {
        frame: remove_frame,
        ..
    } = remove
    else {
        panic!("controller should forward remove-child call");
    };
    runtime
        .receive(&Ingress::Parent, remove_frame)
        .expect("runtime should prune topology");
    assert!(runtime.endpoint().child_routes().is_empty());
}
