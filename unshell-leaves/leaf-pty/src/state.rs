use unshell::protocol::{HookID, unshell_leaf};

use crate::{constants::LEAF_FAKE_PTY, procedure::PingProcedure, session::PtySession};

/// User-owned state for the generated fake PTY leaf.
///
/// The `unshell_leaf!` template stores sessions and retry queues around this struct.
/// Keeping counters here makes tests and future procedures observe leaf behavior
/// without reaching into generated session storage.
pub struct FakePtyState {
    /// Number of sessions that application logic considers active.
    pub active_count: usize,

    /// Total number of successfully opened sessions.
    pub total_opened: u64,

    /// Last hook that received stdin EOF.
    pub last_stdin_eof_hook: Option<HookID>,
}

impl FakePtyState {
    /// Creates a fake PTY state with no active sessions.
    pub fn new() -> Self {
        Self {
            active_count: 0,
            total_opened: 0,
            last_stdin_eof_hook: None,
        }
    }
}

impl Default for FakePtyState {
    fn default() -> Self {
        Self::new()
    }
}

unshell_leaf! {
    pub leaf FakePtyLeaf for FakePtyState {
        id: LEAF_FAKE_PTY,
        meta: unshell::protocol::LeafMeta {
            name: "Fake PTY Leaf",
            identifier: "dev.unshell.v1.pty",
            version: "v0",
            authors: unshell::alloc::vec!["ASTATIN3"],
        },
        sessions {
            pty: PtySession,
        }
        procedures {
            ping: PingProcedure,
        }
    }
}
