#[path = "support/protocol_bench_common.rs"]
mod common;

fn main() {
    let iterations = common::iterations_from_args(1_000);
    common::run_hook_data_receive(iterations);
    println!("hook_data_receive iterations={iterations}");
}
