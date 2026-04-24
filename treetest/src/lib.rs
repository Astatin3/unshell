//! Interactive UnShell protocol demo crate.
//!
//! This crate intentionally keeps protocol logic in the root `unshell` crate and
//! uses that implementation as a consumer would: by building endpoint topologies,
//! simulating packet transport, and rendering an inspector UI around the results.

pub mod app;
pub mod model;
pub mod scenarios;
pub mod sim;

pub use app::run;
