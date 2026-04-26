//! Remote shell leaf and its user-facing surfaces.
//!
//! The module always exports the protocol contract for the leaf. Role-specific
//! implementations live behind crate-wide features:
//! - `leaf_endpoint` builds the PTY-backed runtime leaf
//! - `leaf_tui` builds a placeholder client-side TUI surface

use rkyv::{Archive, Deserialize, Serialize};

#[cfg(feature = "leaf_endpoint")]
pub mod endpoint;
#[cfg(feature = "leaf_tui")]
pub mod tui;

#[cfg(feature = "leaf_endpoint")]
pub use endpoint::RemoteShellEndpoint;
#[cfg(feature = "leaf_tui")]
pub use tui::RemoteShellTui;

/// Open-request payload for the remote shell leaf.
///
/// The shell currently needs no structured arguments, but a named payload type is
/// easier for downstream code to discover than a bare `()`.
#[derive(Archive, Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct OpenRequest;

crate::role_leaf! {
    /// Feature-selected remote shell surface.
    pub type RemoteShell {
        endpoint => endpoint::RemoteShellEndpoint,
        tui => tui::RemoteShellTui,
    }
}
