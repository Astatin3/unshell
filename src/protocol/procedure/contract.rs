use crate::protocol::{Endpoint, Packet, ProcedureOut};

#[cfg(feature = "interface_ratatui")]
use crate::interface::ProcedureView;

/// Contract implemented by one generated one-packet procedure handler.
///
/// Procedures are for stateless or short-lived operations such as ping, capabilities,
/// or health checks. Long-running conversations should use [`Session`](crate::protocol::Session)
/// so final packet cleanup and retries remain tied to hook state.
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
