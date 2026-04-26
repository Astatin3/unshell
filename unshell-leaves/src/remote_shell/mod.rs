//! Remote shell leaf and its user-facing surfaces.
//!
//! The module always exports the protocol contract for the leaf together with the
//! endpoint and TUI host implementations.

use rkyv::{Archive, Deserialize, Serialize};
use unshell_macros::leaf;

pub mod endpoint;
pub mod tui;

/// Open-request payload for the remote shell leaf.
///
/// The shell currently needs no structured arguments, but a named payload type is
/// easier for downstream code to discover than a bare `()`.
#[derive(Archive, Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct OpenRequest;

#[leaf(
    name = "remote_shell",
    procedures = [Open],
    endpoint = endpoint,
    tui = tui,
)]
/// Shared compile-time declaration for the `remote_shell` leaf surface.
pub struct RemoteShell;
