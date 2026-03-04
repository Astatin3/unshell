#![feature(proc_macro_quote)]
#![feature(proc_macro_span)]
#![allow(dead_code, unused_macros, unused_imports)]

mod env;
mod format_helper;
mod proc_impl_switcher;

mod obfuscate;

// Types of symbolic reference
mod symbolic_aes;
mod symbolic_ref;

use proc_macro::TokenStream;
use quote::quote;

use proc_impl_switcher::proc_impl;

#[proc_macro]
pub fn obs(input: TokenStream) -> TokenStream {
    proc_impl::xor(input)
}

#[proc_macro_attribute]
pub fn sym_fn(_attr: TokenStream, item: TokenStream) -> TokenStream {
    proc_impl::sym_fn(item)
}

#[proc_macro]
pub fn sym(input: TokenStream) -> TokenStream {
    proc_impl::sym(input)
}

#[proc_macro]
pub fn junk_asm(input: TokenStream) -> TokenStream {
    proc_impl::junk_asm(input)
}

#[proc_macro]
pub fn file_symbol(_input: TokenStream) -> TokenStream {
    // Get the call site span to extract file information
    let span = proc_macro::Span::call_site();
    let source_file = span.source();
    let file_path = source_file.file();
    let line_num = source_file.line();
    let concatted = format!("{}:{}", file_path, line_num);

    // Return as a string literal
    let output = quote! {
        obfuscate::sym!(#concatted)
    };
    output.into()
}

#[proc_macro]
pub fn sym_format(input: TokenStream) -> TokenStream {
    format_helper::sym_format(input)
}
