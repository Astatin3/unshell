use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn, LitStr};

pub fn xor(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as LitStr);

    (quote::quote! {
        String::from(#input)
    })
    .into()
}
pub fn aes_fn_name(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let func = parse_macro_input!(item as ItemFn);
    TokenStream::from(quote! {
        #[unsafe(no_mangle)]
        #func
    })
}

pub fn aes_str(input: TokenStream) -> TokenStream {
    input
}

pub fn junk_asm(_input: TokenStream) -> TokenStream {
    TokenStream::new()
}
