#[path = "support/protocol_bench_common.rs"]
mod common;

fn main() {
    let iterations = common::iterations_from_args(1_000);
    let checksum = common::run_forward_call_receive(iterations);
    println!("forward_call_receive iterations={iterations} checksum={checksum}");
}
