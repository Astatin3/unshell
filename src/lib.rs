//! # UnShell Core
//!
//! This crate implements the UnShell protocol as a pure, `no_std` library.
//! It provides routed endpoint communication using an explicit tree topology.
//!
//! ## Architecture
//!
//! - [`protocol`] - Wire types, framing, stateless validation, and routing/runtime.
//!
//! The library requires `alloc` for path and payload management.

#![no_std]

pub extern crate alloc;
// Re-export derive macros against a stable `::unshell` path, including when the
// macros are used inside this crate's own examples and tests.
#[allow(unused_extern_crates)]
extern crate self as unshell;

pub mod logger;

/// Re-export the protocol crate behind the historical `unshell::protocol` path so
/// proc-macro output and downstream code do not need a second migration.
pub use unshell_protocol as protocol;

/// Re-export the leaf library crate behind the historical `unshell::leaves` path
pub use unshell_leaves as leaves;

pub use unshell_macros::{Procedure, leaf, procedures};

/// Creates a root-assumed endpoint from one local identifier plus any number of leaf hosts.
///
/// What it is: a convenience macro that builds a `ProtocolEndpoint` whose protocol path starts at
/// root, with no parent or children, and whose leaf inventory is inferred from the supplied host
/// values.
///
/// Why it exists: the common bootstrap case should not require callers to manually construct an
/// empty path, `Vec<ChildRoute>`, and a `Vec<LeafSpec>` when they already have leaf host values.
///
/// # Example
/// ```rust
/// use unshell::{create_endpoint, leaf};
/// use unshell::protocol::tree::Endpoint;
///
/// #[derive(Default)]
/// struct DemoLeaf;
///
/// #[leaf(id = "org.example.v1.demo", procedures = ["ping"], endpoint_struct = DemoLeaf)]
/// struct Demo;
///
/// let endpoint = create_endpoint!("demo", DemoLeaf::default());
/// assert!(endpoint.path().is_empty());
/// assert_eq!(endpoint.local_id(), Some("demo"));
/// ```
#[macro_export]
macro_rules! create_endpoint {
    ($id:expr $(, $leaf:expr )* $(,)?) => {{
        let mut __unshell_leaf_specs = ::unshell::alloc::vec::Vec::new();
        $(
            let __unshell_leaf = $leaf;
            __unshell_leaf_specs.push(::unshell::protocol::tree::leaf_spec_of(&__unshell_leaf));
        )*
        ::unshell::protocol::tree::ProtocolEndpoint::root($id, __unshell_leaf_specs)
    }};
}

// pub use ush_obfuscate as obfuscate;
