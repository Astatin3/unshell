#[path = "support/protocol_bench_common.rs"]
mod common;

fn main() {
    let iterations = common::iterations_from_args(1_000);
    let checksum = common::run_local_call_receive(iterations);
    println!("local_call_receive iterations={iterations} checksum={checksum}");
}
