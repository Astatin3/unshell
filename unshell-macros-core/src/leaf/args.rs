use syn::{
    Expr, Ident, Result, Token, Type,
    parse::{Parse, ParseStream},
};

/// Parsed arguments from `#[unshell_leaf(...)]`.
#[derive(Debug)]
pub(crate) struct UnshellLeafArgs {
    pub(crate) leaf: Ident,
    pub(crate) id: Expr,
    pub(crate) sessions: Vec<Type>,
    pub(crate) procedures: Vec<Type>,
}

impl Parse for UnshellLeafArgs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut leaf = None;
        let mut id = None;
        let mut sessions = Vec::new();
        let mut procedures = Vec::new();

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            match key.to_string().as_str() {
                "leaf" => {
                    reject_duplicate(&leaf, &key)?;
                    input.parse::<Token![=]>()?;
                    leaf = Some(input.parse()?);
                }
                "id" => {
                    reject_duplicate(&id, &key)?;
                    input.parse::<Token![=]>()?;
                    id = Some(input.parse()?);
                }
                "sessions" => {
                    sessions = parse_type_list(input)?;
                }
                "procedures" => {
                    procedures = parse_type_list(input)?;
                }
                _ => {
                    return Err(syn::Error::new(
                        key.span(),
                        "expected `leaf`, `id`, `sessions`, or `procedures`",
                    ));
                }
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(Self {
            leaf: leaf.ok_or_else(|| input.error("missing `leaf = WrapperName`"))?,
            id: id.ok_or_else(|| input.error("missing `id = LEAF_ID`"))?,
            sessions,
            procedures,
        })
    }
}

/// Rejects repeated scalar keys while keeping repeated list keys additive by design.
fn reject_duplicate<T>(slot: &Option<T>, key: &Ident) -> Result<()> {
    if slot.is_some() {
        Err(syn::Error::new(key.span(), "duplicate key"))
    } else {
        Ok(())
    }
}

/// Parses `name(Type, Type)` argument payloads.
fn parse_type_list(input: ParseStream<'_>) -> Result<Vec<Type>> {
    let content;
    syn::parenthesized!(content in input);
    let parsed = content.parse_terminated(Type::parse, Token![,])?;
    Ok(parsed.into_iter().collect())
}
