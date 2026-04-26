use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Error, Ident, LitStr, Result, Token, Visibility,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

use crate::utils::option_litstr_tokens;

pub(crate) struct LeafDeclarationInput {
    visibility: Visibility,
    name: Option<LitStr>,
    id: Option<LitStr>,
    org: Option<LitStr>,
    product: Option<LitStr>,
    version: Option<LitStr>,
    endpoint_struct: Option<Ident>,
    tui_struct: Option<Ident>,
    procedures: Vec<ProcedureRef>,
}

impl Parse for LeafDeclarationInput {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let visibility = if input.peek(Token![pub]) {
            input.parse()?
        } else {
            Visibility::Inherited
        };

        let assignments = Punctuated::<LeafAssignment, Token![,]>::parse_terminated(input)?;
        let mut parsed = Self {
            visibility,
            name: None,
            id: None,
            org: None,
            product: None,
            version: None,
            endpoint_struct: None,
            tui_struct: None,
            procedures: Vec::new(),
        };

        for assignment in assignments {
            match assignment {
                LeafAssignment::Name(value) => {
                    if parsed.name.is_some() {
                        return Err(Error::new_spanned(value, "duplicate leaf name"));
                    }
                    parsed.name = Some(value);
                }
                LeafAssignment::Id(value) => {
                    if parsed.id.is_some() {
                        return Err(Error::new_spanned(value, "duplicate leaf id"));
                    }
                    parsed.id = Some(value);
                }
                LeafAssignment::Org(value) => {
                    if parsed.org.is_some() {
                        return Err(Error::new_spanned(value, "duplicate leaf org"));
                    }
                    parsed.org = Some(value);
                }
                LeafAssignment::Product(value) => {
                    if parsed.product.is_some() {
                        return Err(Error::new_spanned(value, "duplicate leaf product"));
                    }
                    parsed.product = Some(value);
                }
                LeafAssignment::Version(value) => {
                    if parsed.version.is_some() {
                        return Err(Error::new_spanned(value, "duplicate leaf version"));
                    }
                    parsed.version = Some(value);
                }
                LeafAssignment::EndpointStruct(value) => {
                    if parsed.endpoint_struct.is_some() {
                        return Err(Error::new_spanned(value, "duplicate endpoint_struct"));
                    }
                    parsed.endpoint_struct = Some(value);
                }
                LeafAssignment::TuiStruct(value) => {
                    if parsed.tui_struct.is_some() {
                        return Err(Error::new_spanned(value, "duplicate tui_struct"));
                    }
                    parsed.tui_struct = Some(value);
                }
                LeafAssignment::Procedures(values) => {
                    if !parsed.procedures.is_empty() {
                        return Err(Error::new(input.span(), "duplicate procedures list"));
                    }
                    parsed.procedures = values;
                }
            }
        }

        if parsed.name.is_none() && parsed.id.is_none() {
            return Err(Error::new(
                input.span(),
                "leaf! requires either `name = \"...\"` or `id = \"...\"`",
            ));
        }
        if parsed.endpoint_struct.is_none() && parsed.tui_struct.is_none() {
            return Err(Error::new(
                input.span(),
                "leaf! requires at least one of `endpoint_struct = ...` or `tui_struct = ...`",
            ));
        }

        Ok(parsed)
    }
}

enum LeafAssignment {
    Name(LitStr),
    Id(LitStr),
    Org(LitStr),
    Product(LitStr),
    Version(LitStr),
    EndpointStruct(Ident),
    TuiStruct(Ident),
    Procedures(Vec<ProcedureRef>),
}

enum ProcedureRef {
    Symbol(Ident),
    Suffix(LitStr),
}

impl Parse for ProcedureRef {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        if input.peek(LitStr) {
            return Ok(Self::Suffix(input.parse()?));
        }
        Ok(Self::Symbol(input.parse()?))
    }
}

impl Parse for LeafAssignment {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let name: Ident = input.parse()?;
        input.parse::<Token![=]>()?;
        match name.to_string().as_str() {
            "name" => Ok(Self::Name(input.parse()?)),
            "id" => Ok(Self::Id(input.parse()?)),
            "org" => Ok(Self::Org(input.parse()?)),
            "product" => Ok(Self::Product(input.parse()?)),
            "version" => Ok(Self::Version(input.parse()?)),
            "endpoint_struct" => Ok(Self::EndpointStruct(input.parse()?)),
            "tui_struct" => Ok(Self::TuiStruct(input.parse()?)),
            "procedures" => {
                let content;
                syn::bracketed!(content in input);
                let values = Punctuated::<ProcedureRef, Token![,]>::parse_terminated(&content)?
                    .into_iter()
                    .collect::<Vec<_>>();
                Ok(Self::Procedures(values))
            }
            _ => Err(Error::new_spanned(name, "unsupported leaf! assignment")),
        }
    }
}

pub(crate) fn expand_leaf_declaration(input: LeafDeclarationInput) -> Result<TokenStream> {
    let _visibility = input.visibility;
    let declaration_ident = format_ident!(
        "__UnshellLeafDecl_{}",
        input
            .endpoint_struct
            .as_ref()
            .or(input.tui_struct.as_ref())
            .expect("leaf declaration requires at least one host")
    );
    let id = option_litstr_tokens(input.id.as_ref());
    let org = option_litstr_tokens(input.org.as_ref());
    let product = option_litstr_tokens(input.product.as_ref());
    let version = option_litstr_tokens(input.version.as_ref());
    let leaf_name = option_litstr_tokens(input.name.as_ref());
    let procedure_suffixes = input
        .procedures
        .iter()
        .map(|procedure| match procedure {
            ProcedureRef::Symbol(procedure) => {
                quote! { <#procedure as ::unshell::protocol::tree::ProcedureMetadata>::PROCEDURE_SUFFIX }
            }
            ProcedureRef::Suffix(suffix) => quote! { #suffix },
        })
        .collect::<Vec<_>>();
    let procedure_type_checks = input
        .procedures
        .iter()
        .filter_map(|procedure| match procedure {
            ProcedureRef::Symbol(procedure) => Some(procedure),
            ProcedureRef::Suffix(_) => None,
        });

    let endpoint_impl = input
        .endpoint_struct
        .as_ref()
        .map(|endpoint_struct| expand_binding_impl(endpoint_struct, &declaration_ident));
    let tui_impl = input
        .tui_struct
        .as_ref()
        .map(|tui_struct| expand_binding_impl(tui_struct, &declaration_ident));

    Ok(quote! {
        #[allow(non_camel_case_types)]
        #[doc(hidden)]
        pub struct #declaration_ident;

        impl ::unshell::protocol::tree::ProtocolLeaf for #declaration_ident {
            fn leaf_name() -> ::unshell::alloc::string::String {
                ::unshell::protocol::tree::derive_leaf_name(
                    ::core::env!("CARGO_PKG_NAME"),
                    ::core::env!("CARGO_PKG_VERSION_MAJOR"),
                    ::core::env!("CARGO_PKG_VERSION_MINOR"),
                    ::core::env!("CARGO_PKG_VERSION_PATCH"),
                    ::core::module_path!(),
                    ::core::stringify!(#declaration_ident),
                    #org,
                    #product,
                    #version,
                    #leaf_name,
                    #id,
                )
            }
        }

        impl ::unshell::protocol::tree::LeafDeclaration for #declaration_ident {
            fn procedure_suffixes() -> &'static [&'static str] {
                &[#(#procedure_suffixes),*]
            }
        }

        impl #declaration_ident {
            /// Returns the canonical dotted leaf name declared for this surface.
            pub fn protocol_leaf_name() -> ::unshell::alloc::string::String {
                <Self as ::unshell::protocol::tree::ProtocolLeaf>::leaf_name()
            }

            /// Returns the canonical protocol leaf metadata for this surface.
            pub fn protocol_leaf_spec() -> ::unshell::protocol::tree::LeafSpec {
                <Self as ::unshell::protocol::tree::LeafDeclaration>::leaf_spec()
            }

            /// Resolves one local procedure suffix to its full canonical `procedure_id`.
            pub fn protocol_procedure_id(
                suffix: &str,
            ) -> ::core::option::Option<::unshell::alloc::string::String> {
                <Self as ::unshell::protocol::tree::LeafDeclaration>::procedure_id(suffix)
            }
        }

        const _: fn() = || {
            #(let _ = ::core::marker::PhantomData::<#procedure_type_checks>;)*
        };

        #endpoint_impl
        #tui_impl
    })
}

fn expand_binding_impl(host: &Ident, declaration: &Ident) -> TokenStream {
    quote! {
        impl ::unshell::protocol::tree::ProtocolLeaf for #host {
            fn leaf_name() -> ::unshell::alloc::string::String {
                <#declaration as ::unshell::protocol::tree::ProtocolLeaf>::leaf_name()
            }
        }

        impl ::unshell::protocol::tree::LeafBinding for #host {
            type Declaration = #declaration;
        }

        impl ::unshell::protocol::tree::LeafDeclaration for #host {
            fn procedure_suffixes() -> &'static [&'static str] {
                <#declaration as ::unshell::protocol::tree::LeafDeclaration>::procedure_suffixes()
            }
        }

        impl #host {
            /// Returns the canonical dotted leaf name declared for this host.
            pub fn protocol_leaf_name() -> ::unshell::alloc::string::String {
                <Self as ::unshell::protocol::tree::ProtocolLeaf>::leaf_name()
            }

            /// Returns the canonical protocol leaf metadata for this host.
            pub fn protocol_leaf_spec() -> ::unshell::protocol::tree::LeafSpec {
                <Self as ::unshell::protocol::tree::LeafDeclaration>::leaf_spec()
            }

            /// Resolves one local procedure suffix to its full canonical `procedure_id`.
            pub fn protocol_procedure_id(
                suffix: &str,
            ) -> ::core::option::Option<::unshell::alloc::string::String> {
                <Self as ::unshell::protocol::tree::LeafDeclaration>::procedure_id(suffix)
            }
        }
    }
}
