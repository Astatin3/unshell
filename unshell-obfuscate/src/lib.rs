#![feature(proc_macro_quote)]
#![feature(proc_macro_span)]

use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;

mod format_helper;
use format_helper::*;

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
    obs::obs(input)
}

#[proc_macro_attribute]
pub fn obfuscated_symbol(_attr: TokenStream, item: TokenStream) -> TokenStream {
    obs::obfuscated_symbol(_attr, item)
}

#[proc_macro]
pub fn symbol(input: TokenStream) -> TokenStream {
    obs::symbol(input)
}

#[proc_macro]
pub fn junk_asm(input: TokenStream) -> TokenStream {
    obs::junk_asm(input)
}

//

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
