use std::hint::black_box;
use std::time::Instant;

use unshell::protocol::tree::{
    ChildRoute, Endpoint, Ingress, LeafSpec, LocalEvent, ProtocolEndpoint,
};
use unshell::protocol::{CallMessage, PacketHeader, PacketType, decode_frame, encode_packet};

const SAMPLES: usize = 500;
const ITERS: usize = 1_000;

fn main() {
    println!("protocol benchmark");
    println!("samples: {SAMPLES}");
    println!("iterations/sample: {ITERS}");
    println!();

    let benches = [
        bench_encode_call(),
        bench_decode_call(),
        bench_forward_call_receive(),
        bench_local_call_receive(),
        bench_hook_data_receive(),
    ];

    println!(
        "{:32} {:>14} {:>14} {:>14}",
        "benchmark", "mean ns/op", "stddev", "samples"
    );
    for bench in benches {
        println!(
            "{:32} {:>14.2} {:>14.2} {:>14}",
            bench.name, bench.mean_ns, bench.stddev_ns, bench.samples
        );
    }
}

struct BenchResult {
    name: &'static str,
    mean_ns: f64,
    stddev_ns: f64,
    samples: usize,
}

fn bench_encode_call() -> BenchResult {
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

    run_bench("encode_call", || {
        let frame =
            encode_packet(black_box(&header), black_box(&message)).expect("encode should work");
        black_box(frame.len());
    })
}

fn bench_decode_call() -> BenchResult {
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

    run_bench("decode_call", || {
        let parsed = decode_frame(black_box(frame.as_slice())).expect("decode should work");
        let call = parsed.deserialize_call().expect("call should deserialize");
        black_box(call.data.len());
    })
}

fn bench_forward_call_receive() -> BenchResult {
    run_prebuilt_bench(
        "forward_call_receive",
        build_forward_call_cases,
        |(mut root, frame)| {
            let outcome = root
                .receive(&Ingress::Local, frame)
                .expect("forward receive should work");
            black_box(outcome.forward.is_some());
        },
    )
}

fn bench_local_call_receive() -> BenchResult {
    run_prebuilt_bench(
        "local_call_receive",
        build_local_call_cases,
        |(mut endpoint, frame)| {
            let outcome = endpoint
                .receive(&Ingress::Parent, frame)
                .expect("local call should work");
            match black_box(outcome.event) {
                Some(LocalEvent::Call { .. }) => {}
                other => panic!("expected local call event, got {other:?}"),
            }
        },
    )
}

fn bench_hook_data_receive() -> BenchResult {
    run_prebuilt_bench(
        "hook_data_receive",
        build_hook_data_cases,
        |(mut host, frame)| {
            let outcome = host
                .receive(&Ingress::Child(path(&["worker"])), frame)
                .expect("hook data should work");
            match black_box(outcome.event) {
                Some(LocalEvent::Data { .. }) => {}
                other => panic!("expected local data event, got {other:?}"),
            }
        },
    )
}

fn run_bench(name: &'static str, mut op: impl FnMut()) -> BenchResult {
    let mut samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let start = Instant::now();
        for _ in 0..ITERS {
            op();
        }
        let elapsed = start.elapsed().as_nanos() as f64 / ITERS as f64;
        samples.push(elapsed);
    }
    summarize(name, &samples)
}

fn run_prebuilt_bench<T, F>(
    name: &'static str,
    mut build_cases: F,
    mut op: impl FnMut(T),
) -> BenchResult
where
    F: FnMut() -> Vec<T>,
{
    let mut repeated = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let mut cases = build_cases();
        assert_eq!(cases.len(), ITERS);
        let start = Instant::now();
        for case in cases.drain(..) {
            op(case);
        }
        let elapsed = start.elapsed().as_nanos() as f64 / ITERS as f64;
        repeated.push(elapsed);
    }
    summarize(name, &repeated)
}

fn build_forward_call_cases() -> Vec<(ProtocolEndpoint, unshell::protocol::FrameBytes)> {
    (0..ITERS)
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

fn build_local_call_cases() -> Vec<(ProtocolEndpoint, unshell::protocol::FrameBytes)> {
    (0..ITERS)
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

fn build_hook_data_cases() -> Vec<(ProtocolEndpoint, unshell::protocol::FrameBytes)> {
    (0..ITERS)
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

fn summarize(name: &'static str, samples: &[f64]) -> BenchResult {
    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    let variance = samples
        .iter()
        .map(|sample| {
            let delta = sample - mean;
            delta * delta
        })
        .sum::<f64>()
        / samples.len() as f64;

    BenchResult {
        name,
        mean_ns: mean,
        stddev_ns: variance.sqrt(),
        samples: samples.len(),
    }
}

fn path(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| String::from(*part)).collect()
}
