use std::collections::HashMap;

use base62::{Base62, hash};
use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, LitStr, parse_macro_input};

use crate::env::get_encryption_key;

static mut SYM_COUNTER: Vec<String> = Vec::new();

#[allow(static_mut_refs)]
pub fn get_symbol_number() -> usize {
    unsafe { SYM_COUNTER.len() }
}

#[allow(static_mut_refs)]
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

fn ref_string(input: String) -> String {
    let n = get_symbol(&input);

    let data = base62::encode_usize(n);
    let key = hash(&get_encryption_key().as_bytes());

    let encoded = format!("_{}_", Base62::encode_full(&data, &key));

    println!("Aliased '{}' as '{encoded}'", input);

    encoded
}

pub fn sym_ref(input: TokenStream) -> TokenStream {
    // Parse the input as a string literal
    let lit_str = parse_macro_input!(input as LitStr);
    let original_name = lit_str.value();

    let encoded = ref_string(original_name);

    // Expand to a static string literal
    TokenStream::from(quote! {
        #encoded
    })
}

pub fn sym_ref_fn(input: TokenStream) -> TokenStream {
    // Parse the input function
    let func = parse_macro_input!(input as ItemFn);

    // Get the original function name
    let fn_name = func.sig.ident.to_string();

    // Generate the new, obfuscated name
    let obfuscated_name = ref_string(fn_name);

    // Create a new string literal for the name
    let new_name_lit = LitStr::new(&obfuscated_name, func.sig.ident.span());

    // Re-build the function, but add #[no_mangle]
    // and rename the *exported* symbol via #[export_name]
    TokenStream::from(quote! {
        #[unsafe(export_name = #new_name_lit)]
        #func
    })
}
