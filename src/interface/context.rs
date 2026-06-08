/// Services available to one interface update pass.
///
/// The context is intentionally service-oriented instead of state-oriented. Leaves use
/// the database to persist serialized session/procedure state and, when the Ratatui
/// backend is enabled, use the renderer service to draw the current and historical
/// state in the same pass. The context never owns decoded session objects; decoded
/// meaning stays inside the leaf implementation that produced the bytes.
pub struct InterfaceContext<'a> {
    /// Caller-owned storage for serialized leaf internals.
    pub database: &'a mut dyn crate::interface::InterfaceDatabase,

    /// Optional caller-supplied timestamp for serialized state writes.
    ///
    /// The current blob database does not impose a record format, so this value is
    /// available to handwritten leaves that include timestamps inside their own bytes.
    /// Generated leaves do not write it automatically because that would create a
    /// hidden schema around otherwise opaque serialized sessions.
    pub now_ns: Option<u64>,

    /// Ratatui renderer used by generated leaves for the shared leaf chrome.
    ///
    /// The concrete implementation lives outside the core protocol crate. Keeping it
    /// as a trait object lets `unshell-tui` provide terminal UI behavior without
    /// making the storage format or generated leaf API depend on a specific renderer.
    #[cfg(feature = "interface_ratatui")]
    pub ratatui: &'a mut dyn crate::interface::RatatuiInterface,
}
