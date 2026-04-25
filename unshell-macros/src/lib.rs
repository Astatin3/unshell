//! Proc macros for `unshell` application-layer leaf declarations.

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    DeriveInput, Error, Ident, LitStr, Result, Token, parse::Parse, parse_macro_input,
    punctuated::Punctuated,
};

#[proc_macro_derive(Leaf, attributes(leaf))]
pub fn derive_leaf(input: TokenStream) -> TokenStream {
    match expand_leaf(parse_macro_input!(input as DeriveInput)) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn expand_leaf(input: DeriveInput) -> Result<proc_macro2::TokenStream> {
    let struct_name = input.ident;
    match input.data {
        syn::Data::Struct(_) => {}
        _ => {
            return Err(Error::new_spanned(
                struct_name,
                "Leaf can only be derived for structs",
            ));
        }
    };

    let parsed = LeafAttributes::parse_from(&input.attrs)?;
    let procedures = parsed.procedures.clone().ok_or_else(|| {
        Error::new_spanned(&struct_name, "missing #[leaf(procedures(...))] attribute")
    })?;

    if procedures.is_empty() {
        return Err(Error::new_spanned(
            &struct_name,
            "leaf must declare at least one procedure suffix",
        ));
    }

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let leaf_name_expr = parsed.leaf_name_expression(&struct_name);
    let procedure_suffix_literals = procedures
        .iter()
        .map(|procedure| LitStr::new(&procedure.to_string(), proc_macro2::Span::call_site()))
        .collect::<Vec<_>>();
    let warning_note = parsed
        .explicit_id_value()
        .as_ref()
        .filter(|name| !name.value().is_empty())
        .filter(|name| !looks_like_canonical_leaf_name(&name.value()))
        .map(|name| {
            LitStr::new(
                &format!(
                    "leaf id `{}` does not follow the recommended dotted format `org.product.vN.leaf_name[.part]`",
                    name.value()
                ),
                proc_macro2::Span::call_site(),
            )
        })
        .map(|note| {
            let attr = quote! { #[deprecated(note = #note)] };
            (attr.clone(), attr.clone(), attr)
        });
    let (leaf_spec_warning_attr, procedure_warning_attr, leaf_name_warning_attr) =
        warning_note.unwrap_or_else(|| (quote! {}, quote! {}, quote! {}));

    Ok(quote! {
        impl #impl_generics ::unshell::protocol::tree::ProtocolLeaf for #struct_name #ty_generics #where_clause {
            fn leaf_name() -> ::unshell::alloc::string::String {
                #leaf_name_expr
            }

            fn procedure_suffixes() -> &'static [&'static str] {
                &[#(#procedure_suffix_literals),*]
            }
        }

        impl #impl_generics #struct_name #ty_generics #where_clause {
            /// Returns the canonical protocol leaf metadata for this type.
            #leaf_spec_warning_attr
            pub fn protocol_leaf_spec() -> ::unshell::protocol::tree::LeafSpec {
                <Self as ::unshell::protocol::tree::ProtocolLeaf>::leaf_spec()
            }

            /// Resolves one local procedure suffix to its full canonical `procedure_id`.
            #procedure_warning_attr
            pub fn protocol_procedure_id(
                suffix: &str,
            ) -> ::core::option::Option<::unshell::alloc::string::String> {
                <Self as ::unshell::protocol::tree::ProtocolLeaf>::procedure_id(suffix)
            }

            /// Returns the canonical dotted leaf name declared for this type.
            #leaf_name_warning_attr
            pub fn protocol_leaf_name() -> ::unshell::alloc::string::String {
                <Self as ::unshell::protocol::tree::ProtocolLeaf>::leaf_name()
            }
        }
    })
}

#[derive(Default)]
struct LeafAttributes {
    name: Option<LitStr>,
    id: Option<LitStr>,
    org: Option<LitStr>,
    product: Option<LitStr>,
    version: Option<LitStr>,
    leaf_name: Option<LitStr>,
    procedures: Option<Vec<Ident>>,
}

impl LeafAttributes {
    fn parse_from(attrs: &[syn::Attribute]) -> Result<Self> {
        let mut parsed = Self::default();

        for attr in attrs {
            if !attr.path().is_ident("leaf") {
                continue;
            }

            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("name") {
                    if parsed.name.is_some() {
                        return Err(meta.error("duplicate leaf name attribute"));
                    }
                    parsed.name = Some(meta.value()?.parse()?);
                    return Ok(());
                }

                if meta.path.is_ident("id") {
                    if parsed.id.is_some() {
                        return Err(meta.error("duplicate leaf id attribute"));
                    }
                    parsed.id = Some(meta.value()?.parse()?);
                    return Ok(());
                }

                if meta.path.is_ident("org") {
                    if parsed.org.is_some() {
                        return Err(meta.error("duplicate leaf org attribute"));
                    }
                    parsed.org = Some(meta.value()?.parse()?);
                    return Ok(());
                }

                if meta.path.is_ident("product") {
                    if parsed.product.is_some() {
                        return Err(meta.error("duplicate leaf product attribute"));
                    }
                    parsed.product = Some(meta.value()?.parse()?);
                    return Ok(());
                }

                if meta.path.is_ident("version") {
                    if parsed.version.is_some() {
                        return Err(meta.error("duplicate leaf version attribute"));
                    }
                    parsed.version = Some(meta.value()?.parse()?);
                    return Ok(());
                }

                if meta.path.is_ident("leaf_name") {
                    if parsed.leaf_name.is_some() {
                        return Err(meta.error("duplicate leaf_name attribute"));
                    }
                    parsed.leaf_name = Some(meta.value()?.parse()?);
                    return Ok(());
                }

                if meta.path.is_ident("procedures") {
                    if parsed.procedures.is_some() {
                        return Err(meta.error("duplicate leaf procedures attribute"));
                    }

                    let nested: ProcedureList = meta.input.parse()?;
                    parsed.procedures = Some(nested.0.into_iter().collect());
                    return Ok(());
                }

                Err(meta.error("unsupported #[leaf(...)] attribute"))
            })?;
        }

        Ok(parsed)
    }

    fn explicit_id_value(&self) -> Option<&LitStr> {
        self.id.as_ref().or(self.name.as_ref())
    }

    fn leaf_name_expression(&self, struct_name: &Ident) -> proc_macro2::TokenStream {
        let id = option_litstr_tokens(self.id.as_ref().or(self.name.as_ref()));
        let org = option_litstr_tokens(self.org.as_ref());
        let product = option_litstr_tokens(self.product.as_ref());
        let version = option_litstr_tokens(self.version.as_ref());
        let leaf_name = option_litstr_tokens(self.leaf_name.as_ref());

        quote! {
            ::unshell::protocol::tree::derive_leaf_name(
                ::core::env!("CARGO_PKG_NAME"),
                ::core::env!("CARGO_PKG_VERSION_MAJOR"),
                ::core::env!("CARGO_PKG_VERSION_MINOR"),
                ::core::env!("CARGO_PKG_VERSION_PATCH"),
                ::core::module_path!(),
                ::core::stringify!(#struct_name),
                #org,
                #product,
                #version,
                #leaf_name,
                #id,
            )
        }
    }
}

fn option_litstr_tokens(value: Option<&LitStr>) -> proc_macro2::TokenStream {
    match value {
        Some(value) => quote! { ::core::option::Option::Some(#value) },
        None => quote! { ::core::option::Option::None },
    }
}

fn looks_like_canonical_leaf_name(name: &str) -> bool {
    let segments = name.split('.').collect::<Vec<_>>();
    if segments.len() < 4 {
        return false;
    }

    for segment in &segments {
        if segment.is_empty() {
            return false;
        }

        if !segment.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        }) {
            return false;
        }
    }

    if !segments[2].starts_with('v') || segments[2].len() <= 1 {
        return false;
    }

    segments[2][1..]
        .chars()
        .all(|character| character.is_ascii_digit() || character == '_')
}

#[cfg(test)]
mod tests {
    use super::looks_like_canonical_leaf_name;

    #[test]
    fn canonical_leaf_name_accepts_minimal_valid_shape() {
        assert!(looks_like_canonical_leaf_name("org.example.v1.echo"));
        assert!(looks_like_canonical_leaf_name("org.example.v1.echo.abc123"));
    }

    #[test]
    fn canonical_leaf_name_rejects_wrong_shapes() {
        assert!(!looks_like_canonical_leaf_name("org.example.echo"));
        assert!(!looks_like_canonical_leaf_name("org.example.1.echo"));
        assert!(!looks_like_canonical_leaf_name("Org.example.v1.echo"));
    }
}

struct ProcedureList(Punctuated<Ident, Token![,]>);

impl Parse for ProcedureList {
    fn parse(input: syn::parse::ParseStream<'_>) -> Result<Self> {
        let content;
        syn::parenthesized!(content in input);
        Ok(Self(Punctuated::parse_terminated(&content)?))
    }
}
