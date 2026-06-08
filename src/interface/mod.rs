//! Interface services shared by optional operator-facing frontends.
//!
//! This module deliberately exposes storage as a tiny namespaced blob database. The
//! database does not know about packets, audit events, sessions, procedures, or UI
//! widgets. Generated and handwritten leaves own that meaning by serializing their
//! own state into bytes and deserializing those bytes when a frontend asks to display
//! historical work.

mod context;
mod database;
mod key;

#[cfg(feature = "interface_ratatui")]
mod ratatui;

pub use context::InterfaceContext;
pub use database::InterfaceDatabase;
pub use key::{hook_key, procedure_namespace, session_namespace, static_key};

#[cfg(feature = "interface_ratatui")]
pub use ratatui::{RatatuiInterface, RatatuiLeafAreas};
