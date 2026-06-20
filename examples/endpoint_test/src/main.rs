#![no_std]
#![no_main]

use core::net::Ipv4Addr;

use leaf_shell::{ShellLeaf, ShellState};
use tcp_simple::TCPServerLeaf;
use unshell::protocol::{Endpoint, Leaf};

const ID: u32 = 0x12345678;
const CHILD_ID: u32 = 0x87654321;

#[unsafe(no_mangle)]
pub fn main(_argc: i32, _argv: *const *const u8) {
    let mut endpoint = Endpoint::new(ID);
    endpoint.path.push(ID);

    let mut shell = ShellLeaf::new(ShellState::new());
    let mut tcp = TCPServerLeaf::bind_ipv4(Ipv4Addr::LOCALHOST, 1337, CHILD_ID).unwrap();

    loop {
        // One transport tick keeps the minimized binary smaller. Packets read from TCP
        // are processed by the shell on the next loop, then flushed by this same tick.
        shell.update(&mut endpoint);
        tcp.update(&mut endpoint);
    }
}
