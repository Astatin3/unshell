use unshell::protocol::{HookID, unshell_leaf};

use crate::{constants::LEAF_FAKE_PTY, session::PtySession};

/// User-owned state for the generated fake PTY leaf.
///
/// The macro-generated `FakePtyLeaf` wrapper stores sessions and retry queues around
/// this struct. Keeping counters here makes tests and future procedures observe leaf
/// behavior without reaching into generated session storage.
#[unshell_leaf(leaf = FakePtyLeaf, id = LEAF_FAKE_PTY, sessions(PtySession))]
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
