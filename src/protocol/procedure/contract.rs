use crate::protocol::{Endpoint, Packet, ProcedureOut};

#[cfg(feature = "interface_ratatui")]
use crate::interface::InterfaceContext;

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

    /// Renders procedure-owned interface state inside its leaf UI.
    ///
    /// Procedures are usually short-lived, so the first interface pass does not impose
    /// a procedure history format. Leaf authors can still use the context database
    /// directly from custom leaf code, and generated procedure helpers can grow typed
    /// serialization later without changing the leaf-level interface method.
    #[cfg(feature = "interface_ratatui")]
    fn render_interface_ratatui(
        _: &L,
        _: &mut InterfaceContext<'_>,
        _: &mut ratatui::Frame<'_>,
        _: ratatui::layout::Rect,
    ) {
    }
}
