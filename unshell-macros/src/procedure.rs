use quote::quote;
use syn::{Attribute, Data, DeriveInput, Error, LitStr, Result, Type};

#[derive(Default)]
struct ProcedureAttributes {
    leaf: Option<Type>,
    name: Option<LitStr>,
}

impl ProcedureAttributes {
    fn parse_from(attrs: &[Attribute]) -> Result<Self> {
        let mut parsed = Self::default();

        for attr in attrs {
            if !attr.path().is_ident("procedure") {
                continue;
            }

            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("leaf") {
                    if parsed.leaf.is_some() {
                        return Err(meta.error("duplicate procedure leaf attribute"));
                    }
                    parsed.leaf = Some(meta.value()?.parse()?);
                    return Ok(());
                }

                if meta.path.is_ident("name") {
                    if parsed.name.is_some() {
                        return Err(meta.error("duplicate procedure name attribute"));
                    }
                    parsed.name = Some(meta.value()?.parse()?);
                    return Ok(());
                }

                Err(meta.error("unsupported #[procedure(...)] attribute"))
            })?;
        }

        Ok(parsed)
    }
}

pub(crate) fn expand_procedure(input: DeriveInput) -> Result<proc_macro2::TokenStream> {
    let procedure_name = input.ident;
    match input.data {
        Data::Struct(_) => {}
        _ => {
            return Err(Error::new_spanned(
                procedure_name,
                "Procedure can only be derived for structs",
            ));
        }
    };

    let parsed = ProcedureAttributes::parse_from(&input.attrs)?;
    let leaf_ty = parsed.leaf.ok_or_else(|| {
        Error::new_spanned(
            &procedure_name,
            "missing #[procedure(leaf = LeafType, name = \"...\")] attribute",
        )
    })?;
    let suffix = parsed.name.ok_or_else(|| {
        Error::new_spanned(
            &procedure_name,
            "missing #[procedure(leaf = LeafType, name = \"...\")] attribute",
        )
    })?;
    if suffix.value().is_empty() {
        return Err(Error::new_spanned(
            &suffix,
            "procedure name must not be empty",
        ));
    }
    if suffix.value().contains('.') {
        return Err(Error::new_spanned(
            &suffix,
            "procedure name must be one local suffix without dots",
        ));
    }
    if suffix.value().chars().any(char::is_whitespace) {
        return Err(Error::new_spanned(
            &suffix,
            "procedure name must not contain whitespace",
        ));
    }

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    Ok(quote! {
        impl #impl_generics ::unshell::protocol::tree::ProcedureMetadata
            for #procedure_name #ty_generics #where_clause
        where
            #leaf_ty: ::unshell::protocol::tree::ProtocolLeaf,
        {
            type Leaf = #leaf_ty;

            fn procedure_suffix() -> &'static str {
                #suffix
            }
        }

        impl #impl_generics #procedure_name #ty_generics #where_clause {
            /// Returns the full canonical `procedure_id` for this stateful procedure.
            pub fn protocol_procedure_id() -> ::unshell::alloc::string::String {
                <Self as ::unshell::protocol::tree::ProcedureMetadata>::procedure_id()
            }
        }
    })
}
