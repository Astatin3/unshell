use getrandom::fill;
use proc_macro::TokenStream;
use quote::quote;
use syn::{LitStr, parse_macro_input};

/// XOR encrypt strings
pub fn xor(input: TokenStream) -> TokenStream {
    // Parse the input as a string literal
    let lit_str = parse_macro_input!(input as LitStr);
    let original_str = lit_str.value();

    // Handle empty strings explicitly
    if original_str.is_empty() {
        return TokenStream::from(quote! { String::new() });
    }

    // --- Obfuscated Branch Logic ---
    // This code runs at compile-time

    let str_bytes = original_str.as_bytes();
    let len = str_bytes.len();

    // 1. Generate a unique, random key for this string
    let mut key = vec![0u8; len];
    fill(&mut key).expect("Failed to get random bytes for XOR key");

    // 2. XOR the string with the key
    let mut obfuscated = Vec::with_capacity(len);
    for i in 0..len {
        obfuscated.push(str_bytes[i] ^ key[i]);
    }

    // 3. This is the code that will be injected into the user's binary
    //    It runs at *runtime* to decrypt the string.
    let obfuscated_expansion = quote! {
        {
            // These static arrays are stored directly in your binary
            static OBFUSCATED_DATA: [u8; #len] = [ #( #obfuscated ),* ];
            static KEY_DATA: [u8; #len] = [ #( #key ),* ];

            let mut decrypted = Vec::with_capacity(#len);
            for i in 0..#len {
                decrypted.push(OBFUSCATED_DATA[i] ^ KEY_DATA[i]);
            }

            // We can trust this since the source was a valid String literal
            String::from_utf8(decrypted).unwrap()
        }
    };

    TokenStream::from(obfuscated_expansion)
}
