//! Proc macros for `unshell` application-layer leaf declarations.

mod leaf;
mod procedure;
mod procedures;
mod utils;

use proc_macro::TokenStream;
use syn::{DeriveInput, ItemImpl, parse_macro_input};

#[proc_macro_derive(Leaf, attributes(leaf))]
pub fn derive_leaf(input: TokenStream) -> TokenStream {
    match leaf::expand_leaf(parse_macro_input!(input as DeriveInput)) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

#[proc_macro_derive(Procedure, attributes(procedure))]
pub fn derive_procedure(input: TokenStream) -> TokenStream {
    match procedure::expand_procedure(parse_macro_input!(input as DeriveInput)) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

#[proc_macro_attribute]
pub fn procedures(attr: TokenStream, item: TokenStream) -> TokenStream {
    match procedures::expand_procedures(
        parse_macro_input!(attr as procedures::ProceduresAttributes),
        parse_macro_input!(item as ItemImpl),
    ) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}
