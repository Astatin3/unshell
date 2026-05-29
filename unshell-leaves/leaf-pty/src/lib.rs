//! PTY leaf support for UnShell.
//!
//! This crate currently contains a deterministic fake PTY session used to prove the
//! macro-generated leaf shape. The fake leaf exercises the same hook-backed protocol
//! invariants as a real PTY worker without pulling OS-specific PTY code into
//! `unshell-protocol`.

#![no_std]

extern crate alloc;

mod codec;
mod constants;
mod session;
mod state;

pub use codec::{
    decode_open_reply_path, encode_frame, encode_open, frame_opcode, frame_payload,
    pty_open_packet, pty_packet,
};
pub use constants::*;
pub use session::{PtySession, PtySessionState};
pub use state::{FakePtyLeaf, FakePtyState};

#[cfg(test)]
mod tests;
