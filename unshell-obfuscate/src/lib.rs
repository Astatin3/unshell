#![feature(proc_macro_quote)]
#![feature(proc_macro_span)]

use proc_macro::TokenStream;
use quote::quote;
use syn::{Expr, ItemFn, LitStr, parse_macro_input};

#[cfg(feature = "obfuscate")]
mod encrypt;

mod format_helper;
use format_helper::*;

// Put all encrypt-related dependencies in a module, so they are easier to use with the feature flag
#[cfg(feature = "obfuscate")]
mod obs_deps {
    pub use crate::encrypt::get_obfuscated_symbol_name;
    pub use syn::LitStr;

    pub const ENV_KEY_NAME: &str = "OBFUSCATION_KEY";
    pub const BACKUP_ENV_KEY: &str = "OBFUSCATION_KEY_DO_NOT_USE";
}
#[cfg(feature = "obfuscate")]
use obs_deps::*;

#[proc_macro]
#[cfg(not(feature = "obfuscate"))]
pub fn obs(input: TokenStream) -> TokenStream {
    input
}

#[proc_macro_attribute]
#[cfg(not(feature = "obfuscate"))]
pub fn obfuscated_symbol(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let func = parse_macro_input!(item as ItemFn);
    TokenStream::from(quote! {
        #[unsafe(no_mangle)]
        #func
    })
}

#[proc_macro]
#[cfg(not(feature = "obfuscate"))]
pub fn symbol(input: TokenStream) -> TokenStream {
    input
}

#[proc_macro_attribute]
#[cfg(feature = "obfuscate")]
pub fn obfuscated_symbol(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the input function
    let func = parse_macro_input!(item as ItemFn);

    // Get the original function name
    let fn_name = func.sig.ident.to_string();

    // Generate the new, obfuscated name
    let obfuscated_name = get_obfuscated_symbol_name(&fn_name);

    // Create a new string literal for the name
    let new_name_lit = LitStr::new(&obfuscated_name, func.sig.ident.span());

    // Re-build the function, but add #[no_mangle]
    // and rename the *exported* symbol via #[export_name]
    TokenStream::from(quote! {
        #[unsafe(export_name = #new_name_lit)]
        #func
    })
}

// --- NEW MACRO 2: The macro for the loader ---

#[proc_macro]
#[cfg(feature = "obfuscate")]
pub fn symbol(input: TokenStream) -> TokenStream {
    // Parse the input as a string literal
    let lit_str = parse_macro_input!(input as LitStr);
    let original_name = lit_str.value();

    // Generate the exact same obfuscated name
    let obfuscated_name = get_obfuscated_symbol_name(&original_name);

    // Expand to a static string literal
    TokenStream::from(quote! {
        #obfuscated_name
    })
}

#[proc_macro]
#[cfg(feature = "obfuscate")]
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
    getrandom::fill(&mut key).expect("Failed to get random bytes for XOR key");

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

// #[proc_macro]
// pub fn file_literal(_input: TokenStream) -> TokenStream {
//     // let input = input.into_iter().collect::<Vec<_>>();
//     // if input.len() != 1 {
//     //     let msg = format!("expected exactly one input token, got {}", input.len());
//     //     return quote! { compile_error!(#msg) }.into();
//     // }

//     let string = file!();
//     let lit_str = LitStr::new(string, proc_macro2::Span::call_site());

//     // let string_lit = match LitStr::try_from(&input) {
//     //     // Error if the token is not a string literal
//     //     Err(e) => return e.to_compile_error(),
//     //     Ok(lit) => lit,
//     // };

//     (quote! {
//         #lit_str
//     })
//     .into()
// }

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
        unshell_obfuscate::symbol!(#concatted)
    };
    // let output = quote! {
    //     #concatted
    // };
    output.into()
}

#[proc_macro]
pub fn format_obs(input: TokenStream) -> TokenStream {
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
                    unshell_obfuscate::symbol!(#text).to_string()
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
