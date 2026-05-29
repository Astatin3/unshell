//! Leaf wrapper macro implementation.
//!
//! Everything in this module is specific to `#[unshell_leaf]`: argument parsing,
//! generated wrapper storage, static dispatch, and retry-safe session output. Future
//! macro families should be added as sibling modules instead of sharing this internal
//! structure.

mod args;
mod generator;
mod names;

use proc_macro2::TokenStream;
use syn::{ItemStruct, Result, parse2};

pub(crate) use args::UnshellLeafArgs;
pub(crate) use generator::LeafGenerator;

/// Expands `#[unshell_leaf(...)]` into a wrapper leaf and `Leaf` implementation.
///
/// Errors are returned as tokenized `compile_error!` output so the proc-macro shim can
/// stay a thin transport layer from compiler tokens to this core implementation.
pub fn expand_unshell_leaf(attr: TokenStream, item: TokenStream) -> TokenStream {
    match expand_unshell_leaf_result(attr, item) {
        Ok(tokens) => tokens,
        Err(error) => error.to_compile_error(),
    }
}

/// Fallible expansion path used by unit tests.
pub fn expand_unshell_leaf_result(attr: TokenStream, item: TokenStream) -> Result<TokenStream> {
    let args = parse2::<UnshellLeafArgs>(attr)?;
    let state = parse2::<ItemStruct>(item)?;
    LeafGenerator::new(args, state).expand()
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn parses_leaf_arguments() {
        let args = parse2::<UnshellLeafArgs>(quote! {
            leaf = DemoLeaf,
            id = 42,
            sessions(DemoSession),
            procedures(PingProcedure)
        })
        .unwrap();

        assert_eq!(args.leaf, "DemoLeaf");
        assert_eq!(args.sessions.len(), 1);
        assert_eq!(args.procedures.len(), 1);
    }

    #[test]
    fn missing_leaf_is_rejected() {
        let error = parse2::<UnshellLeafArgs>(quote! { id = 42 }).unwrap_err();

        assert!(error.to_string().contains("missing `leaf"));
    }

    #[test]
    fn expansion_contains_static_dispatch() {
        let expanded = expand_unshell_leaf_result(
            quote! { leaf = DemoLeaf, id = 9, sessions(DemoSession) },
            quote! { pub struct DemoState; },
        )
        .unwrap()
        .to_string();

        assert!(expanded.contains("struct DemoLeaf"));
        assert!(expanded.contains("impl :: unshell :: protocol :: Leaf for DemoLeaf"));
        assert!(expanded.contains("DemoSession"));
    }
}
