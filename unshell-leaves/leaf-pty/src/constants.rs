use unshell::hash_32;

/// Leaf id used by the generated fake PTY wrapper.
pub const LEAF_FAKE_PTY: u32 = hash_32!("dev.unshell.v1.pty");

/// Outer procedure id used by all fake PTY session packets.
pub const PROC_PTY: u32 = hash_32!("dev.unshell.v1.pty.pty");

/// One-shot procedure id used by tests to prove procedure interface ownership.
pub(crate) const PROC_PING: u32 = hash_32!("dev.unshell.v1.pty.ping");

/// Downward opcode that opens one PTY session.
pub const OP_OPEN: u8 = 0;

/// Upward opcode acknowledging an opened PTY session.
pub const OP_OPENED: u8 = 1;

/// Downward opcode carrying PTY stdin bytes.
pub const OP_INPUT: u8 = 2;

/// Downward opcode representing terminal resize.
pub const OP_RESIZE: u8 = 3;

/// Downward opcode closing PTY stdin without closing the session hook.
pub const OP_STDIN_EOF: u8 = 4;

/// Downward opcode asking the remote process to terminate gracefully.
pub const OP_TERMINATE: u8 = 5;

/// Downward opcode aborting the session without an acknowledgement.
pub const OP_ABORT: u8 = 6;

/// Upward opcode carrying PTY stdout/stderr bytes.
pub const OP_OUTPUT: u8 = 7;

/// Upward final opcode carrying the process exit status.
pub const OP_EXIT: u8 = 8;

/// Upward final opcode carrying a fatal PTY protocol error.
pub const OP_ERROR: u8 = 9;
