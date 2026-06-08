use alloc::collections::VecDeque;

use crate::protocol::{Packet, PacketQueue};

/// Retry queue shared by generated leaves.
///
/// Sessions route directly through `Endpoint` to keep their runtime shape small. This
/// queue remains only for one-shot procedures, whose handlers still use `ProcedureOut`
/// and should not route while the procedure is borrowing leaf state.
pub struct LeafOutbox {
    pub(super) packets: VecDeque<LeafOutboxEntry>,
}

/// One packet retained by a leaf-level retry queue.
///
/// Procedure responses from different generated branches share one queue. The queue
/// deliberately stores only packets; frontend rendering should derive from leaf-owned
/// state rather than from retry internals.
#[derive(Clone)]
pub(super) struct LeafOutboxEntry {
    pub(super) packet: Packet,
}

impl LeafOutbox {
    /// Creates an empty leaf-level outbox.
    pub fn new() -> Self {
        Self {
            packets: VecDeque::new(),
        }
    }

    /// Adds one packet to the retry queue.
    pub fn push(&mut self, packet: Packet) {
        self.packets.push_back(LeafOutboxEntry { packet });
    }

    /// Adds all packets from `packets` in FIFO order.
    pub fn extend(&mut self, packets: PacketQueue) {
        for packet in packets {
            self.push(packet);
        }
    }

    /// Returns the number of queued packets.
    pub fn len(&self) -> usize {
        self.packets.len()
    }

    /// Returns true when the queue has no pending packets.
    pub fn is_empty(&self) -> bool {
        self.packets.is_empty()
    }
}

impl Default for LeafOutbox {
    fn default() -> Self {
        Self::new()
    }
}
