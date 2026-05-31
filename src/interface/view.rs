use alloc::vec::Vec;

use crate::protocol::SessionStatus;

/// Caller-owned render view for one hook-backed session.
pub struct SessionView {
    /// Latest known lifecycle status.
    pub status: SessionViewStatus,

    /// Indices into the store's append-only event list.
    pub events: Vec<usize>,
}

impl SessionView {
    /// Creates an empty pending view.
    pub(crate) fn new() -> Self {
        Self {
            status: SessionViewStatus::Pending,
            events: Vec::new(),
        }
    }
}

/// Caller-owned render view for one one-shot procedure family.
pub struct ProcedureView {
    /// Indices into the store's append-only event list.
    pub events: Vec<usize>,
}

impl ProcedureView {
    /// Creates an empty procedure view.
    pub(crate) fn new() -> Self {
        Self { events: Vec::new() }
    }
}

/// Interface lifecycle state for one session view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionViewStatus {
    /// The view exists because packets referenced it, but no live state exists yet.
    Pending,

    /// The session is active.
    Running,

    /// The session is winding down but may still emit packets.
    Closing,

    /// The session reported that application work is complete.
    Closed,

    /// The leaf rejected the packet that would have created this session.
    Rejected,
}

impl SessionViewStatus {
    /// Converts the protocol session status into a renderable status.
    pub(crate) fn from_session_status(status: SessionStatus) -> Self {
        match status {
            SessionStatus::Running => Self::Running,
            SessionStatus::Closing => Self::Closing,
            SessionStatus::Closed => Self::Closed,
        }
    }
}
