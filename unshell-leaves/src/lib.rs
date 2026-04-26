//! Application-layer leaves and user-facing surfaces built on top of the UnShell
//! protocol runtime.
//!
//! Each leaf module always exports its shared protocol-facing types. Role-specific
//! implementations are selected with the crate-wide `endpoint` and `tui`
//! features, and can optionally be re-exported behind one stable alias.

use unshell::protocol::DataMessage;

/// Re-exports one role-specific type behind a stable public alias.
///
/// This keeps consumers on a single name such as `RemoteShell` while still
/// compiling only the role implementation needed by the current binary.
#[macro_export]
macro_rules! role_leaf {
    (
        $(#[$meta:meta])*
        $vis:vis type $alias:ident {
            endpoint => $endpoint:path,
            tui => $tui:path $(,)?
        }
    ) => {
        #[cfg(all(feature = "endpoint", feature = "tui"))]
        compile_error!(concat!(
            "`",
            stringify!($alias),
            "` can only alias one concrete role at a time; enable either `endpoint` or `tui`, not both"
        ));

        #[cfg(feature = "endpoint")]
        $(#[$meta])*
        $vis type $alias = $endpoint;

        #[cfg(all(not(feature = "endpoint"), feature = "tui"))]
        $(#[$meta])*
        $vis type $alias = $tui;
    };
}

/// Minimal leaf-specific TUI contract.
///
/// The initial implementation intentionally stays transport-agnostic. A CLI can
/// feed validated protocol `DataMessage` values into a leaf TUI and ask it for a
/// textual frame without depending on a specific rendering crate yet.
pub trait LeafTui {
    /// Returns the canonical protocol leaf name this UI understands.
    fn leaf_name(&self) -> String;

    /// Applies one inbound hook payload to the local UI state.
    fn handle_data(&mut self, message: &DataMessage) -> Result<(), TuiError>;

    /// Produces the current textual frame for the leaf.
    fn render(&self) -> String;
}

/// Lightweight error used by the leaf TUI surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuiError {
    message: String,
}

impl TuiError {
    /// Creates one UI-surface error from owned text.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl core::fmt::Display for TuiError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.message)
    }
}

impl core::error::Error for TuiError {}

pub mod remote_shell;
