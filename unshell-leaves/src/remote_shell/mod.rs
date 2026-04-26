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

#[cfg(any(feature = "leaf_endpoint", feature = "leaf_tui"))]
macro_rules! declare_remote_shell_leaf {
    ($($role_args:tt)*) => {
        crate::leaf! {
            name = "remote_shell",
            procedures = [Open],
            $($role_args)*
        }
    };
}

#[cfg(all(feature = "leaf_endpoint", not(feature = "leaf_tui")))]
declare_remote_shell_leaf!(endpoint_struct = RemoteShellEndpoint,);

#[cfg(all(not(feature = "leaf_endpoint"), feature = "leaf_tui"))]
declare_remote_shell_leaf!(tui_struct = RemoteShellTui,);

#[cfg(all(feature = "leaf_endpoint", feature = "leaf_tui"))]
declare_remote_shell_leaf!(
    endpoint_struct = RemoteShellEndpoint,
    tui_struct = RemoteShellTui,
);

crate::role_leaf! {
    /// Feature-selected remote shell surface.
    pub type RemoteShell {
        endpoint => endpoint::RemoteShellEndpoint,
        tui => tui::RemoteShellTui,
    }
}
