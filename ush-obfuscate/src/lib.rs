#![feature(proc_macro_quote)]
#![feature(proc_macro_span)]

use proc_macro::TokenStream;
use quote::quote;

mod env;
mod format_helper;

#[allow(dead_code, unused_imports)]
mod no_obfuscate;

#[allow(dead_code, unused_imports)]
mod obfuscate;

#[cfg(not(feature = "obfuscate"))]
use no_obfuscate as obs;
#[cfg(feature = "obfuscate")]
use obfuscate as obs;

// String obfuscation

#[proc_macro]
pub fn obs(input: TokenStream) -> TokenStream {
    obs::xor(input)
}

#[proc_macro_attribute]
pub fn obfuscated_symbol(_attr: TokenStream, item: TokenStream) -> TokenStream {
    obs::aes_fn_name(_attr, item)
}

#[proc_macro]
pub fn symbol(input: TokenStream) -> TokenStream {
    obs::aes_str(input)
}

#[proc_macro]
pub fn junk_asm(input: TokenStream) -> TokenStream {
    obs::junk_asm(input)
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
        obfuscate::symbol!(#concatted)
    };
    output.into()
}

#[proc_macro]
pub fn format_obs(input: TokenStream) -> TokenStream {
    format_helper::format_obs(input)
}
