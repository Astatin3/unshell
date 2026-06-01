use alloc::vec::Vec;

use crate::protocol::Packet;

use super::{
    rpc::OutgoingFrame,
    tree::{BlockChunk, ChildSummary},
};

/// Caller-side synchronization phase.
///
/// This is the manual state machine a future macro should be able to derive from
/// RPC declarations. Each awaiting state owns the partial stream it is collecting,
/// making it clear which packets are legal at each step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum CallerPhase {
    NeedRoot,
    AwaitRoot {
        hook_id: u16,
    },
    Ready,
    AwaitChildren {
        hook_id: u16,
        node_id: u32,
        entries: Vec<ChildSummary>,
    },
    AwaitBlock {
        hook_id: u16,
        block_id: u32,
        chunks: Vec<BlockChunk>,
    },
    Done,
}

/// Test-visible caller observations.
///
/// The harness keeps a shared report handle so assertions can inspect caller
/// behavior without borrowing the concrete leaf for the duration of a protocol run.
#[derive(Debug, Default)]
pub(super) struct CallerReport {
    pub(super) done: bool,
    pub(super) requested_procedures: Vec<u32>,
    pub(super) received_procedures: Vec<u32>,
    pub(super) synchronized_blocks: Vec<u32>,
    pub(super) applied_block_chunks: Vec<(u32, Vec<Vec<u8>>)>,
    pub(super) final_root_hash: Option<u32>,
}

/// Test-visible respondent observations.
#[derive(Debug, Default)]
pub(super) struct RespondentReport {
    pub(super) requests_seen: Vec<u32>,
    pub(super) streams_started: usize,
    pub(super) streams_completed: usize,
    pub(super) frames_sent: usize,
}

/// Respondent-owned response stream.
///
/// It stores encoded frames and exposes packet construction one frame at a time.
/// Since `next_packet` does not advance, a failed route can be retried by calling it
/// again on the next loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ResponseStream {
    hook_id: u16,
    frames: Vec<OutgoingFrame>,
    next_index: usize,
}

impl ResponseStream {
    /// Creates a response stream for one request hook.
    pub(super) fn new(hook_id: u16, frames: Vec<OutgoingFrame>) -> Self {
        Self {
            hook_id,
            frames,
            next_index: 0,
        }
    }

    /// Builds the next packet without advancing the stream.
    pub(super) fn next_packet(&self) -> Option<Packet> {
        let frame = self.frames.get(self.next_index)?;
        Some(frame.to_packet(self.hook_id, self.next_index + 1 == self.frames.len()))
    }

    /// Marks the current frame as successfully sent.
    pub(super) fn advance(&mut self) {
        self.next_index += 1;
    }

    /// Returns true once every frame has been sent.
    pub(super) fn is_complete(&self) -> bool {
        self.next_index >= self.frames.len()
    }

    /// Returns true when the request generated no frames.
    pub(super) fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
}
