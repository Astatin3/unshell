/// Call some other function
macro_rules! passthrough {
    ($name:tt, $ref:expr) => {
        pub fn $name(input: TokenStream) -> TokenStream {
            $ref(input)
        }
    };
}

/// Just return the underlying string
macro_rules! unwrap_string {
    ($func:tt) => {
        pub fn $func(input: TokenStream) -> TokenStream {
            let input = parse_macro_input!(input as LitStr);

            (quote::quote! {
                #input
            })
            .into()
        }
    };
}

/// Delete the content
macro_rules! delete {
    ($func:tt) => {
        pub fn $func(_: TokenStream) -> TokenStream {
            (quote::quote! {}).into()
        }
    };
}

#[cfg(all(not(feature = "obfuscate_aes"), not(feature = "obfuscate_ref")))]
pub mod proc_impl {
    use proc_macro::TokenStream;
    use syn::{LitStr, parse_macro_input};

    unwrap_string!(xor);
    delete!(junk_asm);

    unwrap_string!(sym);
    unwrap_string!(sym_fn);
}

#[cfg(all(feature = "obfuscate_aes", not(feature = "obfuscate_ref")))]
pub mod proc_impl {
    use proc_macro::TokenStream;

    passthrough!(xor, crate::obfuscate::xor);
    passthrough!(junk_asm, crate::obfuscate::junk_asm);

    passthrough!(sym, crate::symbolic_aes::aes_str);
    passthrough!(sym_fn, crate::symbolic_aes::aes_fn_name);
}

#[cfg(feature = "obfuscate_ref")]
pub mod proc_impl {
    use proc_macro::TokenStream;

    passthrough!(xor, crate::obfuscate::xor);
    passthrough!(junk_asm, crate::obfuscate::junk_asm);

    // `sym` and `sym_fn` need one concrete strategy. When both feature flags are enabled,
    // prefer symbolic references so `cargo clippy --all-features` still selects a single,
    // deterministic implementation.
    passthrough!(sym, crate::symbolic_ref::sym_ref);
    passthrough!(sym_fn, crate::symbolic_ref::sym_ref_fn);
}
