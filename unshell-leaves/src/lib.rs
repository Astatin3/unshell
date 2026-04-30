//! Application-layer leaves and user-facing surfaces built on top of the UnShell
//! protocol runtime.
//!
//! Each leaf module always exports its shared protocol-facing types. Role-specific
//! implementations are selected with the crate-wide `leaf_endpoint` and `leaf_tui`
//! features, and can optionally be re-exported behind one stable alias.

#[allow(unused_extern_crates)]
extern crate self as unshell;

pub extern crate alloc;

use unshell_protocol::DataMessage;

pub use unshell_macros::{Procedure, leaf, procedures};
pub use unshell_protocol as protocol;

/// Re-exports one role-specific type behind a stable public alias.
///
/// What it is: a small macro that binds one public type alias to either an
/// endpoint-facing leaf host or a TUI-facing leaf host based on active features.
///
/// Why it exists: downstream code should be able to import one stable name such as
/// `RemoteShell` without caring which concrete role implementation was compiled for
/// the current binary.
///
/// # Example
/// ```rust
/// use unshell_leaves::role_leaf;
/// mod endpoint { pub struct DemoEndpoint; }
/// mod tui { pub struct DemoTui; }
/// role_leaf! {
///     pub type DemoLeaf {
///         endpoint => endpoint::DemoEndpoint,
///         tui => tui::DemoTui,
///     }
/// }
/// # #[cfg(feature = "leaf_endpoint")]
/// # let _ = core::marker::PhantomData::<DemoLeaf>;
/// ```
#[macro_export]
macro_rules! role_leaf {
    (
        $(#[$meta:meta])*
        $vis:vis type $alias:ident {
            endpoint => $endpoint:path,
            tui => $tui:path $(,)?
        }
    ) => {
        #[cfg(all(feature = "leaf_endpoint", feature = "leaf_tui"))]
        compile_error!(concat!(
            "`",
            stringify!($alias),
            "` can only alias one concrete role at a time; enable either `leaf_endpoint` or `leaf_tui`, not both"
        ));

        #[cfg(feature = "leaf_endpoint")]
        $(#[$meta])*
        $vis type $alias = $endpoint;

        #[cfg(all(not(feature = "leaf_endpoint"), feature = "leaf_tui"))]
        $(#[$meta])*
        $vis type $alias = $tui;
    };
}

/// Minimal leaf-specific TUI contract.
///
/// What it is: the smallest public trait a leaf-specific user interface needs in
/// order to consume protocol `DataMessage` values and render a textual frame.
///
/// Why it exists: leaf UIs should remain transport-agnostic and renderer-agnostic,
/// so callers can experiment with CLIs and TUIs without coupling the core leaf API
/// to any one terminal framework.
///
/// # Example
/// ```rust
/// use unshell_leaves::{LeafTui, TuiError};
/// use unshell_leaves::protocol::DataMessage;
/// struct DemoTui;
/// impl LeafTui for DemoTui {
///     fn leaf_name(&self) -> String { "org.example.v1.demo".into() }
///     fn handle_data(&mut self, _message: &DataMessage) -> Result<(), TuiError> { Ok(()) }
///     fn render(&self) -> String { String::from("demo") }
/// }
/// assert_eq!(DemoTui.render(), "demo");
/// ```
pub trait LeafTui {
    /// Returns the canonical protocol leaf name this UI understands.
    fn leaf_name(&self) -> String;

    /// Applies one inbound hook payload to the local UI state.
    fn handle_data(&mut self, message: &DataMessage) -> Result<(), TuiError>;

    /// Produces the current textual frame for the leaf.
    fn render(&self) -> String;
}

/// Lightweight error used by the leaf TUI surface.
///
/// What it is: a small owned-string error for UI adapters built on [`LeafTui`].
///
/// Why it exists: the TUI surface should not force downstream UIs into a heavier
/// error dependency just to report leaf-local rendering or decoding failures.
///
/// # Example
/// ```rust
/// use unshell_leaves::TuiError;
/// let error = TuiError::new("invalid frame");
/// assert_eq!(error.to_string(), "invalid frame");
/// ```
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

pub mod crossbeam_channel;
pub mod remote_shell;
