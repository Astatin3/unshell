use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, LitStr, parse_macro_input};

pub fn obs(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as LitStr);

    (quote::quote! {
        String::from(#input)
    })
    .into()
}
pub fn obfuscated_symbol(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let func = parse_macro_input!(item as ItemFn);
    TokenStream::from(quote! {
        #[unsafe(no_mangle)]
        #func
    })
}

pub fn symbol(input: TokenStream) -> TokenStream {
    input
}

pub fn junk_asm(_input: TokenStream) -> TokenStream {
    TokenStream::new()
}
