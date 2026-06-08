//! Terminal-facing interface helpers for UnShell.
//!
//! The core `unshell` crate owns protocol traits and generated leaf integration. This
//! crate provides concrete operator-facing services for local tools: an in-memory blob
//! database and a compact Ratatui chrome renderer. Keeping these pieces outside the
//! protocol crate lets embedded or `no_std` users provide different storage and UI
//! implementations without inheriting CLI policy.

mod database;
mod ratatui;

pub use database::MemoryInterfaceDatabase;
pub use ratatui::{DefaultRatatuiInterface, InterfaceTheme};
