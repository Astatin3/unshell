use quote::quote;
use syn::{Attribute, Data, DeriveInput, Error, Ident, LitStr, Result};

use crate::utils::{looks_like_canonical_leaf_name, option_litstr_tokens};

#[derive(Default)]
struct LeafAttributes {
    name: Option<LitStr>,
    id: Option<LitStr>,
    org: Option<LitStr>,
    product: Option<LitStr>,
    version: Option<LitStr>,
    leaf_name: Option<LitStr>,
}

impl LeafAttributes {
    fn parse_from(attrs: &[Attribute]) -> Result<Self> {
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

pub(crate) fn expand_leaf(input: DeriveInput) -> Result<proc_macro2::TokenStream> {
    let struct_name = input.ident;
    match input.data {
        Data::Struct(_) => {}
        _ => {
            return Err(Error::new_spanned(
                struct_name,
                "Leaf can only be derived for structs",
            ));
        }
    };

    let parsed = LeafAttributes::parse_from(&input.attrs)?;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let leaf_name_expr = parsed.leaf_name_expression(&struct_name);
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
        .map(|note| quote! { #[deprecated(note = #note)] });
    let leaf_name_warning_attr = warning_note.unwrap_or_else(|| quote! {});

    Ok(quote! {
        impl #impl_generics ::unshell::protocol::tree::ProtocolLeaf for #struct_name #ty_generics #where_clause {
            fn leaf_name() -> ::unshell::alloc::string::String {
                #leaf_name_expr
            }
        }

        impl #impl_generics #struct_name #ty_generics #where_clause {
            /// Returns the canonical dotted leaf name declared for this type.
            #leaf_name_warning_attr
            pub fn protocol_leaf_name() -> ::unshell::alloc::string::String {
                <Self as ::unshell::protocol::tree::ProtocolLeaf>::leaf_name()
            }
        }
    })
}
