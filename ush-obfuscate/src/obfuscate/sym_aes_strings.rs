use crate::crypt::{aes_encrypt::encrypt_aes_lines, BACKUP_ENV_KEY, ENV_KEY_NAME, STATIC_IV};
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn, LitStr};

use crate::obfuscate::get_encryption_key;

/// Obfuscate function names by encrypting in AES
pub fn aes_fn_name(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the input function

    let func = parse_macro_input!(item as ItemFn);

    // Get the original function name
    let fn_name = func.sig.ident.to_string();

    // Generate the new, obfuscated name
    let obfuscated_name = encrypt_aes_lines(&fn_name, &get_encryption_key(), STATIC_IV);

    // Create a new string literal for the name
    let new_name_lit = LitStr::new(&obfuscated_name, func.sig.ident.span());

    // Re-build the function, but add #[no_mangle]
    // and rename the *exported* symbol via #[export_name]
    TokenStream::from(quote! {
        #[unsafe(export_name = #new_name_lit)]
        #func
    })
}

/// Obfuscate strings by encrypting in AES
pub fn aes_str(input: TokenStream) -> TokenStream {
    // Parse the input as a string literal
    let lit_str = parse_macro_input!(input as LitStr);
    let original_name = lit_str.value();

    // Generate the exact same obfuscated name
    let obfuscated_name = encrypt_aes_lines(&original_name, &get_encryption_key(), STATIC_IV);

    // Expand to a static string literal
    TokenStream::from(quote! {
        #obfuscated_name
    })
}
