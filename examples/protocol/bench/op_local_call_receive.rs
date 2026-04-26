//! Standalone benchmark binary for `local_call_receive`.

#[path = "support/bench_common.rs"]
mod common;

fn main() {
    let iterations = common::iterations_from_args(1_000);
    let checksum = common::run_local_call_receive(iterations);
    println!("local_call_receive iterations={iterations} checksum={checksum}");
}
