//! Runtime helpers used by generated leaves.
//!
//! The `unshell_leaf!` macro emits static dispatch code and delegates the reusable
//! session, procedure, retry, and interface-observation behavior to this module.
//! Keeping those pieces in normal Rust makes the macro easier to audit and keeps the
//! smallest endpoint builds free of interface-only logging paths.

mod outbox;
mod procedure;
mod session;

#[cfg(feature = "interface")]
mod interface;

pub use outbox::LeafOutbox;
pub use procedure::{dispatch_procedure, flush_leaf_outbox};
pub use session::{dispatch_session, update_session_family};

#[cfg(feature = "interface")]
pub use interface::{
    dispatch_procedure_interface, dispatch_session_interface, flush_leaf_outbox_interface,
    update_session_family_interface,
};
