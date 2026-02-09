use syn::parse::{Parse, ParseStream};
use syn::{Expr, Lit, Token};

pub struct PrintlnArgs {
    pub format_str: String,
    pub args: Vec<Expr>,
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
pub enum FormatSegment {
    Static(String),
    Dynamic(String, usize), // format spec, arg index
}

pub fn parse_format_string(fmt: &str) -> Vec<FormatSegment> {
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
