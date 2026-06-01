use alloc::collections::VecDeque;

#[cfg(feature = "interface")]
use crate::interface::InterfaceTarget;
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
/// Procedure responses from different generated branches share one queue. Storing the
/// owner beside the packet keeps route logging precise without exposing another public
/// queue type.
#[derive(Clone)]
pub(super) struct LeafOutboxEntry {
    pub(super) packet: Packet,
    #[cfg(feature = "interface")]
    pub(super) target: Option<InterfaceTarget>,
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
        self.packets.push_back(LeafOutboxEntry {
            packet,
            #[cfg(feature = "interface")]
            target: None,
        });
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

    /// Adds one packet with a runtime-known interface target.
    #[cfg(feature = "interface")]
    pub(crate) fn push_for_target(&mut self, packet: Packet, target: InterfaceTarget) {
        self.packets.push_back(LeafOutboxEntry {
            packet,
            target: Some(target),
        });
    }

    /// Adds all packets with the same runtime-known interface target.
    #[cfg(feature = "interface")]
    pub(crate) fn extend_for_target(&mut self, packets: PacketQueue, target: InterfaceTarget) {
        for packet in packets {
            self.push_for_target(packet, target);
        }
    }
}

impl Default for LeafOutbox {
    fn default() -> Self {
        Self::new()
    }
}
