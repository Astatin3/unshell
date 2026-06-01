use alloc::vec::Vec;

use crate::protocol::{HookID, Packet, PacketQueue};

/// Output accumulator passed to [`Procedure::handle`](super::Procedure::handle).
pub struct ProcedureOut {
    hook_id: HookID,
    reply_path: Vec<u32>,
    procedure_id: u32,
    outbox: PacketQueue,
}

impl ProcedureOut {
    /// Creates an empty procedure output queue.
    pub fn new(hook_id: HookID, reply_path: Vec<u32>, procedure_id: u32) -> Self {
        Self {
            hook_id,
            reply_path,
            procedure_id,
            outbox: PacketQueue::new(),
        }
    }

    /// Replaces the response path used by later [`Self::send`] calls.
    pub fn set_reply_path(&mut self, reply_path: Vec<u32>) {
        self.reply_path = reply_path;
    }

    /// Queues raw response data without closing the hook.
    pub fn send(&mut self, data: &[u8]) {
        self.send_with_end(data, false);
    }

    /// Queues raw response data that closes the hook after successful routing.
    pub fn send_final(&mut self, data: &[u8]) {
        self.send_with_end(data, true);
    }

    /// Consumes the output accumulator and returns packets for generated retry logic.
    pub fn into_packets(self) -> PacketQueue {
        self.outbox
    }

    fn send_with_end(&mut self, data: &[u8], end_hook: bool) {
        self.outbox.push_back(Packet {
            hook_id: self.hook_id,
            end_hook,
            path: self.reply_path.clone(),
            procedure_id: self.procedure_id,
            data: data.to_vec(),
        });
    }
}
