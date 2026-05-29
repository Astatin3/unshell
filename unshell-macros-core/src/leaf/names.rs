use syn::{Ident, Result, Type};

/// Returns the final path segment for a session type.
pub(crate) fn last_type_ident(ty: &Type) -> Result<Ident> {
    let Type::Path(path) = ty else {
        return Err(syn::Error::new_spanned(
            ty,
            "session types must be named paths",
        ));
    };
    let Some(segment) = path.path.segments.last() else {
        return Err(syn::Error::new_spanned(ty, "session type path is empty"));
    };

    Ok(segment.ident.clone())
}

/// Converts a Rust type name into a snake-case fragment for generated private fields.
pub(crate) fn to_snake_case(name: &str) -> String {
    let mut output = String::with_capacity(name.len());
    let chars: Vec<char> = name.chars().collect();

    for (index, character) in chars.iter().copied().enumerate() {
        if character.is_ascii_uppercase() {
            let previous = index
                .checked_sub(1)
                .and_then(|previous| chars.get(previous));
            let next = chars.get(index + 1);
            let previous_needs_boundary = previous
                .map(|previous| previous.is_ascii_lowercase() || previous.is_ascii_digit())
                .unwrap_or(false);
            let acronym_needs_boundary = previous
                .map(|previous| previous.is_ascii_uppercase())
                .unwrap_or(false)
                && next.map(|next| next.is_ascii_lowercase()).unwrap_or(false);

            if previous_needs_boundary || acronym_needs_boundary {
                output.push('_');
            }
            output.push(character.to_ascii_lowercase());
        } else {
            output.push(character);
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::to_snake_case;

    #[test]
    fn session_store_fields_are_snake_case() {
        assert_eq!(to_snake_case("PtySession"), "pty_session");
        assert_eq!(to_snake_case("HTTPServer"), "http_server");
    }
}
