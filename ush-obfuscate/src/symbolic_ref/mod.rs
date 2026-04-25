use base62::{Base62, hash};
use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, LitStr, parse_macro_input};

use crate::env::get_encryption_key;

static mut SYM_COUNTER: Vec<String> = Vec::new();

#[allow(static_mut_refs)]
/// Returns how many unique symbols have been registered in this macro process.
pub fn get_symbol_number() -> usize {
    unsafe { SYM_COUNTER.len() }
}

#[allow(static_mut_refs)]
/// Returns the stable numeric ID for `text`, inserting it on first use.
pub fn get_symbol(text: &str) -> usize {
    unsafe {
        if let Some(n) = SYM_COUNTER.iter().position(|r| r == text) {
            n
        } else {
            SYM_COUNTER.push(text.to_string());

            SYM_COUNTER.len() - 1
        }
    }
}

fn encode_symbol_reference(symbol: String) -> String {
    let symbol_index = get_symbol(&symbol);

    let data = base62::encode_usize(symbol_index);
    let key = hash(get_encryption_key().as_bytes());

    let encoded = format!("_{}_", Base62::encode_full(&data, &key));

    // Macro expansion logs make it easier to correlate exported symbols with their aliases.
    println!("Aliased '{}' as '{encoded}'", symbol);

    encoded
}

/// Replaces a string literal with its symbolic reference alias.
pub fn sym_ref(input: TokenStream) -> TokenStream {
    let lit_str = parse_macro_input!(input as LitStr);
    let original_name = lit_str.value();

    let encoded = encode_symbol_reference(original_name);

    TokenStream::from(quote! {
        #encoded
    })
}

/// Re-exports a function under a symbolic reference alias.
pub fn sym_ref_fn(input: TokenStream) -> TokenStream {
    let func = parse_macro_input!(input as ItemFn);
    let fn_name = func.sig.ident.to_string();

    let obfuscated_name = encode_symbol_reference(fn_name);
    let new_name_lit = LitStr::new(&obfuscated_name, func.sig.ident.span());

    TokenStream::from(quote! {
        #[unsafe(export_name = #new_name_lit)]
        #func
    })
}
