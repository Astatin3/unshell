#[path = "support/protocol_bench_common.rs"]
mod common;

fn main() {
    let iterations = common::iterations_from_args(1_000);
    let checksum = common::run_decode_call(iterations);
    println!("decode_call iterations={iterations} checksum={checksum}");
}
