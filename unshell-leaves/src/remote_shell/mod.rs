//! Remote shell leaf and its user-facing surfaces.
//!
//! The module always exports the protocol contract for the leaf. Role-specific
//! implementations live behind crate-wide features:
//! - `leaf_endpoint` builds the PTY-backed runtime leaf
//! - `leaf_tui` builds a placeholder client-side TUI surface

use rkyv::{Archive, Deserialize, Serialize};
#[cfg(not(feature = "leaf_endpoint"))]
use std::string::String;

#[cfg(feature = "leaf_endpoint")]
pub mod endpoint;
#[cfg(feature = "leaf_tui")]
pub mod tui;

#[cfg(feature = "leaf_endpoint")]
pub use endpoint::Open;
#[cfg(feature = "leaf_endpoint")]
pub use endpoint::RemoteShellEndpoint;
#[cfg(feature = "leaf_tui")]
pub use tui::RemoteShellTui;

#[cfg(not(feature = "leaf_endpoint"))]
/// Compile-time procedure symbol kept available even when the endpoint runtime is
/// not built, so the leaf declaration still validates its declared inventory.
pub struct Open;

#[cfg(not(feature = "leaf_endpoint"))]
#[doc(hidden)]
pub struct RemoteShellDeclarationPlaceholder;

#[cfg(not(feature = "leaf_endpoint"))]
impl crate::protocol::tree::ProtocolLeaf for RemoteShellDeclarationPlaceholder {
    fn leaf_name() -> String {
        String::from("remote_shell")
    }
}

#[cfg(not(feature = "leaf_endpoint"))]
impl crate::protocol::tree::ProcedureMetadata for Open {
    type Leaf = RemoteShellDeclarationPlaceholder;
    const PROCEDURE_SUFFIX: &'static str = "open";
}

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
