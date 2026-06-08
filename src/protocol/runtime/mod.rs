//! Runtime helpers used by generated leaves.
//!
//! The `unshell_leaf!` macro emits static dispatch code and delegates the reusable
//! session, procedure, and retry behavior to this module. Keeping those pieces in
//! normal Rust makes the macro easier to audit and keeps the smallest endpoint builds
//! free of frontend-only paths.

mod outbox;
mod procedure;
mod session;

pub use outbox::LeafOutbox;
pub use procedure::{dispatch_procedure, flush_leaf_outbox};
pub use session::{dispatch_session, update_session_family};
