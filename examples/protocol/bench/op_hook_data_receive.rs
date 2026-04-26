//! Standalone benchmark binary for `hook_data_receive`.

#[path = "support/bench_common.rs"]
mod common;

fn main() {
    let iterations = common::iterations_from_args(1_000);
    let checksum = common::run_hook_data_receive(iterations);
    println!("hook_data_receive iterations={iterations} checksum={checksum}");
}
