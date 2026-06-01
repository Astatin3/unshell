use alloc::vec::Vec;

use crate::protocol::{Endpoint, HookID, Packet, PacketQueue};

#[cfg(feature = "interface_ratatui")]
use crate::interface::ProcedureView;

/// Contract implemented by one generated one-packet procedure handler.
///
/// Procedures are for stateless or short-lived operations such as ping, capabilities,
/// or health checks. Long-running conversations should use [`Session`] so final
/// packet cleanup and retries remain tied to hook state.
pub trait Procedure<L> {
    /// Outer packet procedure id handled by this procedure.
    const PROCEDURE_ID: u32;

    /// Handles one packet and optionally queues response packets in `out`.
    fn handle(leaf: &mut L, endpoint: &mut Endpoint, packet: Packet, out: &mut ProcedureOut);

    #[cfg(feature = "interface_ratatui")]
    fn render_ratatui(
        _: &L,
        _: &mut ProcedureView,
        _: &mut ratatui::Frame<'_>,
        _: ratatui::layout::Rect,
    ) {
    }
}

/// Output accumulator passed to [`Procedure::handle`].
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
