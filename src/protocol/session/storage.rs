use alloc::vec::Vec;

use crate::protocol::{HookID, PacketQueue};

/// Storage entry used by macro-generated session stores.
///
/// The fields are public so generated code in downstream crates can keep the update
/// loop straightforward and static. Handwritten leaves may also use this type, but it
/// is intentionally small rather than a full session framework.
pub struct SessionEntry<S> {
    /// Hook id associated with this live session.
    pub hook_id: HookID,

    /// Application-owned session state.
    pub state: S,

    /// Packets delivered for this hook but not yet consumed by the session.
    pub inbox: PacketQueue,

    /// Whether application logic has finished and should be removed after update.
    pub closed: bool,
}

/// Generated storage for one session family.
///
/// The macro only names this field and picks the concrete `Session` type. All update,
/// retry, and cleanup behavior lives in normal Rust helpers so the template stays
/// small and readable.
pub struct SessionFamily<S> {
    /// Active hook-backed sessions for this family.
    pub entries: Vec<SessionEntry<S>>,
}

impl<S> SessionFamily<S> {
    /// Creates an empty session family.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Counts packets retained by this family for retry or future session work.
    pub fn pending_packet_count(&self) -> usize {
        let mut count = 0usize;

        for entry in &self.entries {
            count += entry.inbox.len();
        }

        count
    }
}

impl<S> Default for SessionFamily<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> SessionEntry<S> {
    /// Creates one active session entry for `hook_id`.
    pub fn new(hook_id: HookID, state: S) -> Self {
        Self {
            hook_id,
            state,
            inbox: PacketQueue::new(),
            closed: false,
        }
    }
}
