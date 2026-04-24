//! Simulator action entry point.
//!
//! Public simulator behavior is split into request-style actions, stepping, and
//! small query helpers so UI code can depend on focused APIs.

mod calls;
mod driver;
mod queries;
