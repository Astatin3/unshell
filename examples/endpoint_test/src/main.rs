#![no_std]
#![no_main]

extern crate alloc;

use leaf_shell::{ShellLeaf, ShellState};
use unshell::protocol::{Endpoint, Leaf};

const ID: u32 = 0x12345678;

#[unsafe(no_mangle)]
pub fn main(_argc: i32, _argv: *const *const u8) {
    let mut endpoint = Endpoint::new(ID);
    let mut shell = ShellLeaf::new(ShellState::new());

    loop {
        shell.update(&mut endpoint);
    }
}
