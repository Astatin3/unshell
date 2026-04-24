use alloc::{borrow::ToOwned, string::String, vec, vec::Vec};

use crate::protocol::tree::{
    DefaultRouteProvider, Endpoint, Ingress, LeafBehavior, LeafNode, LeafSpec, LocalEvent,
    ProtocolEndpoint, RouteDecision, RouteProvider, TreeNode,
};
use crate::protocol::{
    DataMessage, EndpointIntrospection, FaultMessage, PacketHeader, PacketType, ProtocolFault,
    deserialize_archived_bytes, encode_packet,
};

fn path(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_owned()).collect()
}

#[test]
fn tree_node_paths_flatten_explicitly() {
    let tree = TreeNode::Root {
        children: vec![TreeNode::Endpoint {
            segment: "branch".to_owned(),
            leaves: vec![LeafNode {
                name: "echo".to_owned(),
                procedures: vec!["unshell.echo.v1.alpha.invoke".to_owned()],
            }],
            children: vec![TreeNode::Endpoint {
                segment: "leaf".to_owned(),
                leaves: Vec::new(),
                children: Vec::new(),
            }],
        }],
    };

    assert_eq!(
        tree.paths(),
        vec![
            Vec::<String>::new(),
            path(&["branch"]),
            path(&["branch", "leaf"])
        ]
    );
}

#[test]
fn longest_prefix_routing_prefers_most_specific_child() {
    let provider = DefaultRouteProvider;
    let child_paths = vec![path(&["a"]), path(&["a", "b"]), path(&["x"])];

    assert_eq!(
        provider.route_destination(&Vec::new(), &child_paths, true, &path(&["a", "b", "c"])),
        RouteDecision::Child(1)
    );
    assert_eq!(
        provider.route_destination(&path(&["a"]), &child_paths, true, &path(&["z"])),
        RouteDecision::Parent
    );
}

#[test]
fn protocol_endpoint_introspection_returns_leaf_summary() {
    let mut endpoint = ProtocolEndpoint::new(
        path(&["root"]),
        Some(Vec::new()),
        Vec::new(),
        vec![LeafSpec {
            name: "echo".to_owned(),
            procedures: vec!["unshell.echo.v1.alpha.invoke".to_owned()],
            behavior: LeafBehavior::Echo,
        }],
    );

    let hook_id = endpoint.allocate_hook_id();
    let frame = endpoint
        .make_call(path(&["root"]), None, "", Some(hook_id), Vec::new())
        .expect("introspection call should encode");

    let outcome = endpoint
        .receive(&Ingress::Local, frame)
        .expect("endpoint should handle introspection");

    assert!(outcome.forwards.is_empty());
    assert_eq!(outcome.events.len(), 1);

    let LocalEvent::Data {
        header,
        message: response,
    } = &outcome.events[0]
    else {
        panic!("expected local data event");
    };
    assert_eq!(header.packet_type, PacketType::Data);
    assert_eq!(header.dst_path, path(&["root"]));
    let introspection = deserialize_archived_bytes::<
        crate::protocol::introspection::ArchivedEndpointIntrospection,
        EndpointIntrospection,
    >(&response.data)
    .expect("introspection payload should deserialize");

    assert!(response.end_hook);
    assert_eq!(introspection.leaves.len(), 1);
    assert_eq!(introspection.leaves[0].leaf_name, "echo");
    assert_eq!(
        introspection.leaves[0].procedures,
        vec!["unshell.echo.v1.alpha.invoke".to_owned()]
    );
}

#[test]
fn invalid_hook_peer_emits_local_fault_event() {
    let mut endpoint = ProtocolEndpoint::new(path(&["client"]), None, Vec::new(), Vec::new());
    let hook_id = endpoint.allocate_hook_id();

    endpoint
        .make_call(
            path(&["server"]),
            None,
            "unshell.echo.v1.alpha.invoke",
            Some(hook_id),
            vec![1, 2, 3],
        )
        .expect("call should establish an active hook");

    let frame = encode_packet(
        &PacketHeader {
            packet_type: PacketType::Data,
            src_path: path(&["client"]),
            dst_path: path(&["client"]),
            dst_leaf: None,
            hook_id: Some(hook_id),
        },
        &DataMessage {
            procedure_id: "unshell.echo.v1.alpha.invoke".to_owned(),
            data: vec![9],
            end_hook: false,
        },
    )
    .expect("data frame should encode");

    let outcome = endpoint
        .receive(&Ingress::Local, frame)
        .expect("invalid peer should be handled");

    assert!(outcome.forwards.is_empty());
    assert_eq!(outcome.events.len(), 1);
    assert!(!outcome.dropped);

    match &outcome.events[0] {
        LocalEvent::Fault { header, message } => {
            assert_eq!(header.packet_type, PacketType::Fault);
            assert_eq!(header.hook_id, Some(hook_id));
            assert_eq!(
                message,
                &FaultMessage {
                    fault: ProtocolFault::INVALID_HOOK_PEER,
                }
            );
        }
        other => panic!("expected fault event, got {other:?}"),
    }
}
