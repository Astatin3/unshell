use alloc::vec::Vec;

/// Human-facing metadata for a leaf implementation.
///
/// This is intentionally static text plus an allocated author list. It is only used
/// by interface frontends and diagnostics, not by hot packet routing.
pub struct LeafMeta {
    pub name: &'static str,
    pub identifier: &'static str,
    pub version: &'static str,
    pub authors: Vec<&'static str>,
}

impl LeafMeta {
    /// Builds metadata for leaves that have not opted into a richer interface label.
    pub fn anonymous() -> Self {
        Self {
            name: "Unnamed Leaf",
            identifier: "dev.unshell.unknown",
            version: "v0",
            authors: Vec::new(),
        }
    }
}
