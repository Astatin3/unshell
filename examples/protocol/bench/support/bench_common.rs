#![allow(dead_code)]

use std::hint::black_box;

use unshell::protocol::tree::{
    ChildRoute, Endpoint, EndpointOutcome, Ingress, LeafSpec, LocalEvent, ProtocolEndpoint,
};
use unshell::protocol::{CallMessage, PacketHeader, PacketType, decode_frame, encode_packet};

pub fn iterations_from_args(default: usize) -> usize {
    std::env::args()
        .nth(1)
        .map(|value| {
            value
                .parse::<usize>()
                .expect("iterations must be a positive integer")
        })
        .unwrap_or(default)
}

#[inline(never)]
pub fn run_encode_call(iterations: usize) -> usize {
    let header = PacketHeader {
        packet_type: PacketType::Call,
        src_path: path(&["root"]),
        dst_path: path(&["root", "worker"]),
        dst_leaf: Some(String::from("service")),
        hook_id: None,
    };
    let message = CallMessage {
        procedure_id: String::from("example.service.v1.invoke"),
        data: vec![7; 64],
        response_hook: None,
    };

    let mut checksum = 0usize;
    for _ in 0..iterations {
        let frame =
            encode_packet(black_box(&header), black_box(&message)).expect("encode should work");
        checksum = checksum.wrapping_add(frame.len());
    }
    black_box(checksum)
}

#[inline(never)]
pub fn run_decode_call(iterations: usize) -> usize {
    let header = PacketHeader {
        packet_type: PacketType::Call,
        src_path: path(&["root"]),
        dst_path: path(&["root", "worker"]),
        dst_leaf: Some(String::from("service")),
        hook_id: None,
    };
    let message = CallMessage {
        procedure_id: String::from("example.service.v1.invoke"),
        data: vec![9; 64],
        response_hook: None,
    };
    let frame = encode_packet(&header, &message).expect("seed frame should encode");

    let mut checksum = 0usize;
    for _ in 0..iterations {
        let parsed = decode_frame(black_box(frame.as_slice())).expect("decode should work");
        let call = parsed.deserialize_call().expect("call should deserialize");
        checksum = checksum
            .wrapping_add(call.data.len())
            .wrapping_add(call.procedure_id.len())
            .wrapping_add(call.response_hook.is_some() as usize);
    }
    black_box(checksum)
}

#[inline(never)]
pub fn run_forward_call_receive(iterations: usize) -> usize {
    let cases = build_forward_call_cases(iterations);
    run_cases(cases, |(mut root, frame)| {
        let outcome = root
            .receive(&Ingress::Local, frame)
            .expect("forward receive should work");
        match outcome {
            EndpointOutcome::Forward { route, frame } => {
                route_value(route).wrapping_add(frame.len())
            }
            EndpointOutcome::Local(_) => 0,
            EndpointOutcome::Dropped => usize::from(true),
        }
    })
}

#[inline(never)]
pub fn run_local_call_receive(iterations: usize) -> usize {
    let cases = build_local_call_cases(iterations);
    run_cases(cases, |(mut endpoint, frame)| {
        let outcome = endpoint
            .receive(&Ingress::Parent, frame)
            .expect("local call should work");
        match outcome {
            EndpointOutcome::Local(LocalEvent::Call { header, message }) => header
                .dst_path
                .len()
                .wrapping_add(header.src_path.len())
                .wrapping_add(header.dst_leaf.as_ref().map_or(0, String::len))
                .wrapping_add(message.data.len())
                .wrapping_add(message.procedure_id.len()),
            other => panic!("expected local call event, got {other:?}"),
        }
    })
}

#[inline(never)]
pub fn run_hook_data_receive(iterations: usize) -> usize {
    let cases = build_hook_data_cases(iterations);
    run_cases(cases, |(mut host, frame)| {
        let outcome = host
            .receive(&Ingress::Child(path(&["worker"])), frame)
            .expect("hook data should work");
        match outcome {
            EndpointOutcome::Local(LocalEvent::Data {
                header, message, ..
            }) => (header.hook_id.unwrap_or_default() as usize)
                .wrapping_add(message.data.len())
                .wrapping_add(message.procedure_id.len())
                .wrapping_add(message.end_hook as usize),
            other => panic!("expected local data event, got {other:?}"),
        }
    })
}

fn run_cases<T>(cases: Vec<T>, mut op: impl FnMut(T) -> usize) -> usize {
    let mut checksum = 0usize;
    for case in cases {
        checksum = checksum.wrapping_add(op(case));
    }
    black_box(checksum)
}

fn build_forward_call_cases(
    iterations: usize,
) -> Vec<(ProtocolEndpoint, unshell::protocol::FrameBytes)> {
    (0..iterations)
        .map(|_| {
            let mut root = ProtocolEndpoint::new(
                Vec::new(),
                None,
                vec![ChildRoute::registered(path(&["edge"]))],
                Vec::new(),
            );
            let hook_id = root.allocate_hook_id();
            let frame = root
                .make_call(
                    path(&["edge", "worker"]),
                    Some(String::from("service")),
                    String::from("example.service.v1.invoke"),
                    Some(hook_id),
                    vec![1; 32],
                )
                .expect("seed call should encode");
            (root, frame)
        })
        .collect()
}

fn build_local_call_cases(
    iterations: usize,
) -> Vec<(ProtocolEndpoint, unshell::protocol::FrameBytes)> {
    (0..iterations)
        .map(|_| {
            let endpoint = ProtocolEndpoint::new(
                path(&["worker"]),
                Some(Vec::new()),
                Vec::new(),
                vec![LeafSpec {
                    name: String::from("service"),
                    procedures: vec![String::from("example.service.v1.invoke")],
                }],
            );
            let frame = encode_packet(
                &PacketHeader {
                    packet_type: PacketType::Call,
                    src_path: Vec::new(),
                    dst_path: path(&["worker"]),
                    dst_leaf: Some(String::from("service")),
                    hook_id: None,
                },
                &CallMessage {
                    procedure_id: String::from("example.service.v1.invoke"),
                    data: vec![2; 32],
                    response_hook: Some(unshell::protocol::HookTarget {
                        hook_id: 42,
                        return_path: Vec::new(),
                    }),
                },
            )
            .expect("seed local call should encode");
            (endpoint, frame)
        })
        .collect()
}

fn build_hook_data_cases(
    iterations: usize,
) -> Vec<(ProtocolEndpoint, unshell::protocol::FrameBytes)> {
    (0..iterations)
        .map(|_| {
            let mut host = ProtocolEndpoint::new(
                Vec::new(),
                None,
                vec![ChildRoute::registered(path(&["worker"]))],
                Vec::new(),
            );
            let hook_id = host.allocate_hook_id();
            host.make_call(
                path(&["worker"]),
                None,
                String::from("example.service.v1.invoke"),
                Some(hook_id),
                vec![3; 8],
            )
            .expect("seed active hook should encode");
            let frame = encode_packet(
                &PacketHeader {
                    packet_type: PacketType::Data,
                    src_path: path(&["worker"]),
                    dst_path: Vec::new(),
                    dst_leaf: None,
                    hook_id: Some(hook_id),
                },
                &unshell::protocol::DataMessage {
                    procedure_id: String::from("example.service.v1.invoke"),
                    data: vec![4; 16],
                    end_hook: false,
                },
            )
            .expect("seed data should encode");
            (host, frame)
        })
        .collect()
}

pub fn path(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| String::from(*part)).collect()
}

fn route_value(route: unshell::protocol::tree::RouteDecision) -> usize {
    match route {
        unshell::protocol::tree::RouteDecision::Child(index) => index,
        unshell::protocol::tree::RouteDecision::Local => usize::MAX - 2,
        unshell::protocol::tree::RouteDecision::Parent => usize::MAX - 1,
        unshell::protocol::tree::RouteDecision::Drop => usize::MAX,
    }
}
