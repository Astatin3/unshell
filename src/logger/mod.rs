//! # Logger Module
//!
//! Lightweight logging primitives for `no_std` environments.
//!
//! The logger stays intentionally small:
//! - call sites use exported `debug!`, `info!`, `warn!`, and `error!` macros
//! - sinks implement [`Logger`]
//! - startup code installs a single global logger with [`set_logger`]
//!
//! ## Quick start
//!
//! ```rust
//! use unshell::{error, info, warn};
//! use unshell::logger::{Logger, Record, set_logger};
//!
//! struct Sink;
//!
//! impl Logger for Sink {
//!     fn log(&self, record: &Record<'_>) {
//!         let _ = record;
//!     }
//! }
//!
//! static LOGGER: Sink = Sink;
//! set_logger(&LOGGER);
//!
//! info!("starting up");
//! warn!("slow path engaged");
//! error!("critical failure");
//! ```
//!
//! ## Design notes
//!
//! The global sink is installed once at startup and then treated as immutable.
//! That contract lets the module stay `no_std` while still providing a simple
//! global logging API.

mod global;
mod level;
mod macros;
mod record;
mod sink;

#[cfg(test)]
mod tests;

pub use global::{global_logger, log, set_logger};
pub use level::LogLevel;
pub use record::Record;
pub use sink::{CompatibilityLogger, Logger};
