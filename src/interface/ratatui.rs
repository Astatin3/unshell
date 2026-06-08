/// Areas allocated by the shared Ratatui leaf chrome.
///
/// Generated leaves ask the renderer to draw the outer leaf UI once, then use these
/// areas for active and historical sessions. The renderer owns theme and layout
/// policy, while the generated leaf keeps ownership of typed session rendering.
#[derive(Debug, Clone, Copy)]
pub struct RatatuiLeafAreas {
    /// Area intended for currently active session objects.
    pub active_sessions: ratatui::layout::Rect,

    /// Area intended for deserialized historical session objects.
    pub historical_sessions: ratatui::layout::Rect,
}

/// Ratatui rendering service used by the leaf-level interface pass.
///
/// The trait lives in the core crate only so generated leaves can call it without
/// depending on a TUI crate. Concrete UI behavior belongs in `unshell-tui`, which can
/// implement themes, widgets, focus, and operator input while leaving protocol leaves
/// free of terminal-specific layout code.
pub trait RatatuiInterface {
    /// Draws shared leaf chrome and returns child areas for typed session rendering.
    fn render_leaf_chrome(
        &mut self,
        meta: &crate::protocol::LeafMeta,
        active_session_count: usize,
        historical_session_count: usize,
        frame: &mut ratatui::Frame<'_>,
        area: ratatui::layout::Rect,
    ) -> RatatuiLeafAreas;
}
