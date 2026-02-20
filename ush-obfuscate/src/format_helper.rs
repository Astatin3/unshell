use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Expr, Lit, Token, parse_macro_input};

pub fn sym_format(input: TokenStream) -> TokenStream {
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
                    obfuscate::sym!(#text).to_string()
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

struct PrintlnArgs {
    format_str: String,
    args: Vec<Expr>,
}

impl Parse for PrintlnArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let format_expr: Expr = input.parse()?;

        let format_str = match format_expr {
            Expr::Lit(ref lit) => {
                if let Lit::Str(ref s) = lit.lit {
                    s.value()
                } else {
                    return Err(syn::Error::new_spanned(lit, "Expected string literal"));
                }
            }
            _ => {
                return Err(syn::Error::new_spanned(
                    format_expr,
                    "Expected string literal",
                ));
            }
        };

        let mut args = Vec::new();
        while !input.is_empty() {
            input.parse::<Token![,]>()?;
            if input.is_empty() {
                break;
            }
            args.push(input.parse()?);
        }

        Ok(PrintlnArgs { format_str, args })
    }
}

#[derive(Debug)]
enum FormatSegment {
    Static(String),
    Dynamic(String, usize), // format spec, arg index
}

fn parse_format_string(fmt: &str) -> Vec<FormatSegment> {
    let mut segments = Vec::new();
    let mut current_static = String::new();
    let mut chars = fmt.chars().peekable();
    let mut arg_idx = 0;

    while let Some(ch) = chars.next() {
        if ch == '{' {
            if chars.peek() == Some(&'{') {
                chars.next();
                current_static.push('{');
                continue;
            }

            // Save current static segment
            if !current_static.is_empty() {
                segments.push(FormatSegment::Static(current_static.clone()));
                current_static.clear();
            }

            // Parse format spec
            let mut spec = String::new();
            while let Some(&next_ch) = chars.peek() {
                if next_ch == '}' {
                    chars.next();
                    break;
                }
                spec.push(chars.next().unwrap());
            }

            segments.push(FormatSegment::Dynamic(spec, arg_idx));
            arg_idx += 1;
        } else if ch == '}' {
            if chars.peek() == Some(&'}') {
                chars.next();
                current_static.push('}');
            } else {
                current_static.push(ch);
            }
        } else {
            current_static.push(ch);
        }
    }

    if !current_static.is_empty() {
        segments.push(FormatSegment::Static(current_static));
    }

    segments
}
