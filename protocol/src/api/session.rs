// ===========================================================================
// 2. Session storage (the "lightweight" part)
// ===========================================================================

use std::any::Any;

use crate::api::{HookId, NodeId, ProcedureId};

/// Per-session data Node itself needs to track. Kept minimal on purpose:
/// anything procedure-specific goes in `state`, not here, so adding a
/// new procedure never grows this struct.
pub struct SessionData {
    pub peer: NodeId,
    pub hook: HookId,
    pub procedure: ProcedureId,
    pub state: SessionState,
}

/// Most procedures are stateless across calls (or fit in a few words),
/// so state defaults to inline storage and only pays for a heap
/// allocation when a procedure actually needs an arbitrary type.
pub enum SessionState {
    /// No custom state. The common case -- zero size, zero alloc.
    None,
    /// Small fixed-size inline scratch space (counters, flags, a short
    /// buffer) for procedures that need *some* state but not much.
    /// Size is a design knob; pick the smallest value that covers most
    /// built-in procedures (e.g. 32 bytes) and force outliers to Boxed.
    Inline([u8; 32]),
    /// Escape hatch for procedures with real state. Only this variant
    /// allocates.
    Boxed(Box<dyn Any + Send>),
}

/// Storage strategy for sessions, abstracted so single- and
/// multi-threaded Nodes can each supply their own concurrency wrapper
/// around the same underlying arena logic.
pub trait SessionStore {
    type Key;

    fn insert(&mut self, data: SessionData) -> Self::Key;
    fn get(&self, id: &Self::Key) -> Option<&SessionData>;
    fn get_mut(&mut self, id: &Self::Key) -> Option<&mut SessionData>;
    fn remove(&mut self, id: &Self::Key) -> Option<SessionData>;
}
