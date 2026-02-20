//! Compile-time string obfuscation for stealthy payloads.
//!
//! This crate provides procedural macros for encrypting strings at compile time,
//! making them harder to detect via static analysis.
//!
//! # Features
//!
//! - `obfuscate`: Enable AES encryption (enabled via cargo feature)
//! - When disabled, strings pass through as plain text (for debugging)
//!
//! # Macros
//!
//! - `sym!("string")` - Encrypt a string literal
//! - `xor!("string")` - XOR obfuscate a string
//! - `sym_fn` - Obfuscate function names
//! - `junk_asm` - Insert junk assembly instructions
//! - `file_symbol` - Get obfuscated file location for logging
//! - `format_sym` - Format strings with obfuscation
//!
//! # Usage
//!
//! ```rust
//! use ush_obfuscate::sym;
//!
//! const API_KEY: &str = sym!("SuperSecretKey123");
//! const C2_URL: &str = sym!("https://C2Server/endpoint");
//! ```
//!
//! When `obfuscate` feature is enabled, strings are encrypted at compile time.

#![feature(proc_macro_quote)]
#![feature(proc_macro_span)]

use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;

mod format_helper;
use format_helper::*;

mod crypt;

#[allow(dead_code, unused_imports)]
mod no_obfuscate;

#[allow(dead_code, unused_imports)]
mod obfuscate;

#[cfg(not(feature = "obfuscate"))]
use no_obfuscate as obs;
#[cfg(feature = "obfuscate")]
use obfuscate as obs;

// String obfuscation

/// XOR obfuscate a string at compile time.
///
/// Simple XOR-based encoding for basic obfuscation.
#[proc_macro]
pub fn xor(input: TokenStream) -> TokenStream {
    obs::xor(input)
}

/// Encrypt a string using AES at compile time.
///
/// This is the primary macro for string obfuscation.
/// The string is encrypted with a hardcoded key and decrypted at runtime.
#[proc_macro]
pub fn sym(input: TokenStream) -> TokenStream {
    obs::aes_str(input)
}

/// Obfuscate a function name.
///
/// Can be used to hide function names from static analysis.
#[proc_macro_attribute]
pub fn sym_fn(_attr: TokenStream, item: TokenStream) -> TokenStream {
    obs::aes_fn_name(_attr, item)
}

/// Insert junk assembly instructions.
///
/// Adds random assembly instructions to confuse disassembly.
#[proc_macro]
pub fn junk_asm(input: TokenStream) -> TokenStream {
    obs::junk_asm(input)
}

/// Get obfuscated file location for logging.
///
/// Encodes the file path and line number for debug logging
/// without exposing readable strings in the binary.
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
        sym!(#concatted)
    };
    // let output = quote! {
    //     #concatted
    // };
    output.into()
}

/// Format a string with obfuscated parts.
///
/// Combines format string parsing with string obfuscation.
#[proc_macro]
pub fn format_sym(input: TokenStream) -> TokenStream {
    let PrintlnArgs { format_str, args } = parse_macro_input!(input as PrintlnArgs);

    let segments = parse_format_string(&format_str);

    if segments.is_empty() {
        return quote! {
            print!("\n")
        }
        .into();
    }

    let mut parts = Vec::new();

    for segment in segments {
        match segment {
            FormatSegment::Static(text) => {
                parts.push(quote! {
                    #text.to_string()
                });
            }
            FormatSegment::Dynamic(spec, idx) => {
                if idx >= args.len() {
                    return syn::Error::new(
                        proc_macro2::Span::call_site(),
                        format!("argument {} is missing", idx),
                    )
                    .to_compile_error()
                    .into();
                }

                let arg = &args[idx];
                let fmt_spec = if spec.is_empty() {
                    quote! { "{}" }
                } else {
                    let full_spec = format!("{{{}}}", spec);
                    quote! { #full_spec }
                };

                // quote! {
                //     println!(#fmt_spec, #arg);
                // }
                parts.push(quote! {
                    format!(#fmt_spec, #arg)
                });
            }
        }
    }

    (quote! {
        {
            let mut string = String::new();
            #(
                string.push_str(&#parts);
            )*
            string
        }
    })
    .into()
}
