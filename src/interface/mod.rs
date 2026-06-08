//! Interface extension points for optional operator-facing frontends.
//!
//! The previous interface layer mixed packet tracing, session/procedure view storage,
//! and rendering state into one global store. That design has been removed so leaves
//! can expose backend-specific rendering directly from their own state, matching the
//! rest of the protocol's leaf/session/procedure ownership model.

// TODO(interface-ratatui): add the small Ratatui render context that should be passed
// to `Leaf::render_ratatui`, `Session::render_ratatui`, and
// `Procedure::render_ratatui`. It should describe the render target, not own packet
// history or generated runtime state.
