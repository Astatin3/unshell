//! Parser and code generator for UnShell procedural macros.
//!
//! This crate is intentionally not a proc-macro crate. Keeping each macro family's
//! parser and code generator here makes them unit-testable and prevents parsing
//! dependencies from leaking into runtime crates.

mod leaf;

pub use leaf::{expand_unshell_leaf, expand_unshell_leaf_result};
