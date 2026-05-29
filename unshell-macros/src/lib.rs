//! Procedural macro shim for UnShell.
//!
//! The real parser and code generator live in `unshell-macros-core` so they can be
//! tested as ordinary Rust. This crate only adapts compiler `TokenStream`s.

use proc_macro::TokenStream;

/// Generates an `unshell_protocol::Leaf` wrapper for a user-owned state struct.
///
/// See `LEAF_MACRO_INTERFACE.md` for the design contract. The generated wrapper owns
/// session stores, retry queues, filtered packet dispatch, and final-frame cleanup.
#[proc_macro_attribute]
pub fn unshell_leaf(attr: TokenStream, item: TokenStream) -> TokenStream {
    unshell_macros_core::expand_unshell_leaf(attr.into(), item.into()).into()
}
