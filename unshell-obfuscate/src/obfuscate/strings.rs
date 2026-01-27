use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, LitStr, parse_macro_input};
use unshell_crypt::{BACKUP_ENV_KEY, ENV_KEY_NAME, STATIC_IV, aes::encrypt_aes_lines, fill};

#[cfg(feature = "obfuscate")]
#[static_init::dynamic]
static KEY: String = {
    std::env::var(ENV_KEY_NAME).unwrap_or({
        println!("Using default encryption key!");
        BACKUP_ENV_KEY.to_owned()
    })
};

// If there isn't any encryption
#[cfg(not(feature = "obfuscate"))]
#[static_init::dynamic]
static KEY: String = "".to_string();

pub fn obfuscated_symbol(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the input function

    let func = parse_macro_input!(item as ItemFn);

    // Get the original function name
    let fn_name = func.sig.ident.to_string();

    // Generate the new, obfuscated name
    let obfuscated_name = encrypt_aes_lines(&fn_name, &KEY, STATIC_IV);

    // Create a new string literal for the name
    let new_name_lit = LitStr::new(&obfuscated_name, func.sig.ident.span());

    // Re-build the function, but add #[no_mangle]
    // and rename the *exported* symbol via #[export_name]
    TokenStream::from(quote! {
        #[unsafe(export_name = #new_name_lit)]
        #func
    })
}

pub fn symbol(input: TokenStream) -> TokenStream {
    // Parse the input as a string literal
    let lit_str = parse_macro_input!(input as LitStr);
    let original_name = lit_str.value();

    // Generate the exact same obfuscated name
    let obfuscated_name = encrypt_aes_lines(&original_name, &KEY, STATIC_IV);

    // Expand to a static string literal
    TokenStream::from(quote! {
        #obfuscated_name
    })
}

pub fn obs(input: TokenStream) -> TokenStream {
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
