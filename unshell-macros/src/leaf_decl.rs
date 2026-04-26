use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Error, ItemStruct, LitStr, Path, Result, Token,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

use crate::utils::option_litstr_tokens;

pub(crate) struct LeafDeclarationAttributes {
    name: Option<LitStr>,
    id: Option<LitStr>,
    org: Option<LitStr>,
    product: Option<LitStr>,
    version: Option<LitStr>,
    procedures: Vec<ProcedureRef>,
    host_bindings: Vec<HostBinding>,
}

impl Parse for LeafDeclarationAttributes {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let assignments = Punctuated::<LeafAssignment, Token![,]>::parse_terminated(input)?;
        let mut parsed = Self {
            name: None,
            id: None,
            org: None,
            product: None,
            version: None,
            procedures: Vec::new(),
            host_bindings: Vec::new(),
        };

        for assignment in assignments {
            match assignment {
                LeafAssignment::Name(value) => set_once(&mut parsed.name, value, "leaf name")?,
                LeafAssignment::Id(value) => set_once(&mut parsed.id, value, "leaf id")?,
                LeafAssignment::Org(value) => set_once(&mut parsed.org, value, "leaf org")?,
                LeafAssignment::Product(value) => {
                    set_once(&mut parsed.product, value, "leaf product")?
                }
                LeafAssignment::Version(value) => {
                    set_once(&mut parsed.version, value, "leaf version")?
                }
                LeafAssignment::Procedures(values) => {
                    if !parsed.procedures.is_empty() {
                        return Err(Error::new(input.span(), "duplicate procedures list"));
                    }
                    parsed.procedures = values;
                }
                LeafAssignment::HostBinding(binding) => parsed.host_bindings.push(binding),
            }
        }

        if parsed.name.is_none() && parsed.id.is_none() {
            return Err(Error::new(
                input.span(),
                "#[leaf(...)] requires either `name = \"...\"` or `id = \"...\"`",
            ));
        }
        if parsed.host_bindings.is_empty() {
            return Err(Error::new(
                input.span(),
                "#[leaf(...)] requires at least one host binding",
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
    Procedures(Vec<ProcedureRef>),
    HostBinding(HostBinding),
}

struct HostBinding {
    module_path: Option<Path>,
    host_path: Option<Path>,
}

enum ProcedureRef {
    Symbol(Path),
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
        let name: Path = input.parse()?;
        input.parse::<Token![=]>()?;
        let key = name
            .get_ident()
            .ok_or_else(|| Error::new_spanned(&name, "leaf keys must be identifiers"))?
            .to_string();
        match key.as_str() {
            "name" => Ok(Self::Name(input.parse()?)),
            "id" => Ok(Self::Id(input.parse()?)),
            "org" => Ok(Self::Org(input.parse()?)),
            "product" => Ok(Self::Product(input.parse()?)),
            "version" => Ok(Self::Version(input.parse()?)),
            "endpoint_struct" | "tui_struct" => Ok(Self::HostBinding(HostBinding {
                module_path: None,
                host_path: Some(input.parse()?),
            })),
            "endpoint" | "tui" => Ok(Self::HostBinding(HostBinding {
                module_path: Some(input.parse()?),
                host_path: None,
            })),
            "procedures" => {
                let content;
                syn::bracketed!(content in input);
                let values = Punctuated::<ProcedureRef, Token![,]>::parse_terminated(&content)?
                    .into_iter()
                    .collect::<Vec<_>>();
                Ok(Self::Procedures(values))
            }
            _ => Err(Error::new_spanned(
                name,
                "unsupported #[leaf(...)] key; expected one of name, id, org, product, version, procedures, endpoint, tui, endpoint_struct, or tui_struct",
            )),
        }
    }
}

pub(crate) fn expand_leaf_declaration(
    attr: LeafDeclarationAttributes,
    item: ItemStruct,
) -> Result<TokenStream> {
    let declaration_ident = item.ident.clone();
    let id = option_litstr_tokens(attr.id.as_ref());
    let org = option_litstr_tokens(attr.org.as_ref());
    let product = option_litstr_tokens(attr.product.as_ref());
    let version = option_litstr_tokens(attr.version.as_ref());
    let leaf_name = option_litstr_tokens(attr.name.as_ref());
    let canonical_procedure_module = attr
        .host_bindings
        .iter()
        .find_map(|binding| binding.module_path.as_ref())
        .cloned();
    let procedure_suffixes = attr
        .procedures
        .iter()
        .map(|procedure| procedure_suffix_tokens(procedure, canonical_procedure_module.as_ref()))
        .collect::<Result<Vec<_>>>()?;
    let procedure_type_checks = attr
        .host_bindings
        .iter()
        .map(|binding| procedure_type_check_tokens(binding, &attr.procedures, &declaration_ident))
        .collect::<Result<Vec<_>>>()?;
    let host_impls = attr
        .host_bindings
        .iter()
        .map(|binding| expand_binding_impl(binding, &declaration_ident))
        .collect::<Result<Vec<_>>>()?;

    Ok(quote! {
        #item

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
            #(#procedure_type_checks)*
        };

        #(#host_impls)*
    })
}

fn expand_binding_impl(binding: &HostBinding, declaration: &syn::Ident) -> Result<TokenStream> {
    let host = host_path_for_binding(binding, declaration)?;
    Ok(quote! {
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
    })
}

fn host_path_for_binding(binding: &HostBinding, declaration: &syn::Ident) -> Result<Path> {
    if let Some(path) = &binding.host_path {
        return Ok(path.clone());
    }

    let Some(module_path) = &binding.module_path else {
        return Err(Error::new(
            declaration.span(),
            "leaf binding is missing a host path",
        ));
    };

    let mut path = module_path.clone();
    path.segments.push(format_ident!("{declaration}").into());
    Ok(path)
}

fn procedure_suffix_tokens(
    procedure: &ProcedureRef,
    canonical_module: Option<&Path>,
) -> Result<TokenStream> {
    match procedure {
        ProcedureRef::Symbol(procedure) => {
            let procedure_path = if let Some(module_path) = canonical_module {
                let mut path = module_path.clone();
                let ident = procedure.get_ident().ok_or_else(|| {
                    Error::new_spanned(
                        procedure,
                        "procedure names must be bare identifiers when inferred from a module",
                    )
                })?;
                path.segments.push(ident.clone().into());
                path
            } else {
                procedure.clone()
            };
            Ok(
                quote! { <#procedure_path as ::unshell::protocol::tree::ProcedureMetadata>::PROCEDURE_SUFFIX },
            )
        }
        ProcedureRef::Suffix(suffix) => Ok(quote! { #suffix }),
    }
}

fn procedure_type_check_tokens(
    binding: &HostBinding,
    procedures: &[ProcedureRef],
    declaration: &syn::Ident,
) -> Result<TokenStream> {
    let Some(module_path) = &binding.module_path else {
        return Ok(quote! {});
    };

    let checks = procedures
        .iter()
        .filter_map(|procedure| match procedure {
            ProcedureRef::Symbol(procedure) => Some(procedure),
            ProcedureRef::Suffix(_) => None,
        })
        .map(|procedure| {
            let mut path = module_path.clone();
            let ident = procedure.get_ident().ok_or_else(|| {
                Error::new_spanned(
                    procedure,
                    "procedure names must be bare identifiers when inferred from a module",
                )
            })?;
            path.segments.push(ident.clone().into());
            Ok::<TokenStream, Error>(quote! {
                let _ = ::core::marker::PhantomData::<#path>;
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let _ = declaration;
    Ok(quote! { #(#checks)* })
}

fn set_once(target: &mut Option<LitStr>, value: LitStr, label: &str) -> Result<()> {
    if target.is_some() {
        return Err(Error::new_spanned(value, format!("duplicate {label}")));
    }
    *target = Some(value);
    Ok(())
}
