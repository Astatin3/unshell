use alloc::{borrow::ToOwned, string::String, vec, vec::Vec};

use crate::protocol::tree::{
    ChildRoute, DefaultRouteProvider, Endpoint, Ingress, LeafNode, LeafSpec, LocalEvent,
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
                name: "service".to_owned(),
                procedures: vec!["example.service.v1.invoke".to_owned()],
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
        vec![ChildRoute::registered(path(&["root", "child"]))],
        vec![LeafSpec {
            name: "service".to_owned(),
            procedures: vec!["example.service.v1.invoke".to_owned()],
        }],
    );

    let hook_id = endpoint.allocate_hook_id();
    let frame = endpoint
        .make_call(path(&["root"]), None, "", Some(hook_id), Vec::new())
        .expect("introspection call should encode");

    let outcome = endpoint
        .receive(&Ingress::Local, frame)
        .expect("endpoint should handle introspection");

    assert!(outcome.forward.is_none());

    let LocalEvent::Data {
        header,
        message: response,
    } = outcome.event.as_ref().expect("expected local data event")
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
    assert_eq!(introspection.sub_endpoints, vec!["child".to_owned()]);
    assert_eq!(introspection.leaves.len(), 1);
    assert_eq!(introspection.leaves[0].leaf_name, "service");
    assert_eq!(
        introspection.leaves[0].procedures,
        vec!["example.service.v1.invoke".to_owned()]
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
            "example.service.v1.invoke",
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
            procedure_id: "example.service.v1.invoke".to_owned(),
            data: vec![9],
            end_hook: false,
        },
    )
    .expect("data frame should encode");

    let outcome = endpoint
        .receive(&Ingress::Local, frame)
        .expect("invalid peer should be handled");

    assert!(outcome.forward.is_none());
    assert!(!outcome.dropped);

    match outcome.event.as_ref().expect("expected event") {
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

#[test]
fn hook_closes_only_after_both_sides_end() {
    let mut endpoint = ProtocolEndpoint::new(
        Vec::new(),
        None,
        vec![ChildRoute::registered(path(&["server"]))],
        Vec::new(),
    );
    let hook_id = endpoint.allocate_hook_id();

    endpoint
        .make_call(
            path(&["server"]),
            None,
            "example.service.v1.invoke",
            Some(hook_id),
            vec![1],
        )
        .expect("call should establish an active hook");

    let host_key = crate::protocol::tree::HookKey::new(Vec::new(), hook_id);
    assert!(endpoint.hooks.active(&host_key).is_some());

    endpoint
        .send_data(
            path(&["server"]),
            hook_id,
            "example.service.v1.invoke",
            vec![2],
            true,
        )
        .expect("local end should succeed");
    assert!(endpoint.hooks.active(&host_key).is_some());

    let frame = encode_packet(
        &PacketHeader {
            packet_type: PacketType::Data,
            src_path: path(&["server"]),
            dst_path: Vec::new(),
            dst_leaf: None,
            hook_id: Some(hook_id),
        },
        &DataMessage {
            procedure_id: "example.service.v1.invoke".to_owned(),
            data: vec![3],
            end_hook: true,
        },
    )
    .expect("peer final data should encode");

    endpoint
        .receive(&Ingress::Child(path(&["server"])), frame)
        .expect("peer final data should be handled");
    assert!(endpoint.hooks.active(&host_key).is_none());
}

#[test]
fn pending_hook_fault_is_delivered_before_activation() {
    let mut endpoint = ProtocolEndpoint::new(path(&["server"]), None, Vec::new(), Vec::new());
    let header = PacketHeader {
        packet_type: PacketType::Call,
        src_path: path(&["client"]),
        dst_path: path(&["server"]),
        dst_leaf: None,
        hook_id: None,
    };
    let call = crate::protocol::CallMessage {
        procedure_id: crate::protocol::INTROSPECTION_PROCEDURE_ID.to_owned(),
        data: Vec::new(),
        response_hook: Some(crate::protocol::HookTarget {
            hook_id: 11,
            return_path: path(&["client"]),
        }),
    };

    endpoint
        .hooks
        .insert_pending(crate::protocol::tree::PendingHook {
            return_path: path(&["client"]),
            hook_id: 11,
            caller_src_path: path(&["client"]),
            procedure_id: call.procedure_id.clone(),
            dst_leaf: None,
        })
        .expect("pending hook should insert");

    let outcome = endpoint
        .handle_introspection(
            &header,
            Some(crate::protocol::tree::HookKey::new(path(&["client"]), 11)),
        )
        .expect("introspection should handle pending hook");

    assert!(outcome.forward.is_some() || outcome.event.is_some());
}
