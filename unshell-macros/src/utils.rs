use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, GenericArgument, LitStr, Type, TypePath};

pub(crate) fn option_litstr_tokens(value: Option<&LitStr>) -> TokenStream {
    match value {
        Some(value) => quote! { ::core::option::Option::Some(#value) },
        None => quote! { ::core::option::Option::None },
    }
}

pub(crate) fn looks_like_canonical_leaf_name(name: &str) -> bool {
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

pub(crate) fn extract_outer_type_argument<'a>(ty: &'a Type, expected: &str) -> Option<&'a Type> {
    let Type::Path(TypePath { path, .. }) = ty else {
        return None;
    };
    let segment = path.segments.last()?;
    if segment.ident != expected {
        return None;
    }
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    match arguments.args.first()? {
        GenericArgument::Type(inner) => Some(inner),
        _ => None,
    }
}

pub(crate) fn extract_result_type_arguments(ty: &Type) -> Option<(&Type, &Type)> {
    let Type::Path(TypePath { path, .. }) = ty else {
        return None;
    };
    let segment = path.segments.last()?;
    if segment.ident != "Result" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    let mut args = arguments.args.iter();
    let ok = match args.next()? {
        GenericArgument::Type(value) => value,
        _ => return None,
    };
    let err = match args.next()? {
        GenericArgument::Type(value) => value,
        _ => return None,
    };
    Some((ok, err))
}

pub(crate) fn is_unit_type(ty: &Type) -> bool {
    matches!(ty, Type::Tuple(tuple) if tuple.elems.is_empty())
}

pub(crate) fn take_call_attr(attrs: &mut Vec<Attribute>) -> bool {
    let original_len = attrs.len();
    attrs.retain(|attr| !attr.path().is_ident("call"));
    original_len != attrs.len()
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
