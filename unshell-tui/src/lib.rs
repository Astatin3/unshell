//! Reusable pieces behind the `unshell-tui` binary.
//!
//! The binary owns terminal setup and input handling. This library keeps the concrete
//! interface services testable: an in-memory blob database for local sessions and a
//! default Ratatui renderer for generated leaf chrome.

mod database;
mod ratatui;

pub use database::MemoryInterfaceDatabase;
pub use ratatui::{DefaultRatatuiInterface, InterfaceTheme};
