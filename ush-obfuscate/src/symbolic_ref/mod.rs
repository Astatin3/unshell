use std::collections::HashMap;

use base62::{Base62, hash};
use proc_macro::TokenStream;
use quote::quote;
use syn::{LitStr, parse_macro_input};

use crate::env::get_encryption_key;

static mut SYM_COUNTER: Vec<String> = Vec::new();

#[allow(static_mut_refs)]
pub fn get_symbol_number() -> usize {
    unsafe { SYM_COUNTER.len() }
}

#[allow(static_mut_refs)]
pub fn get_symbol(text: String) -> usize {
    unsafe {
        if let Some(n) = SYM_COUNTER.iter().position(|r| r == &text) {
            n
        } else {
            SYM_COUNTER.push(text);

            SYM_COUNTER.len() - 1
        }
    }
}

pub fn sym_ref(input: TokenStream) -> TokenStream {
    // Parse the input as a string literal
    let lit_str = parse_macro_input!(input as LitStr);
    let original_name = lit_str.value();

    let n = get_symbol(original_name);

    let data = base62::encode_usize(n);
    let key = hash(&get_encryption_key().as_bytes());

    let encoded = format!("_{}_", Base62::encode_full(&data, &key));

    println!("Aliased '{}' as '{encoded}'", lit_str.value());

    // Expand to a static string literal
    TokenStream::from(quote! {
        #encoded
    })
}
