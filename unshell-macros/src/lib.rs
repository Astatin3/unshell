//! Proc macros for `unshell` application-layer leaf declarations.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Attribute, DeriveInput, Error, FnArg, GenericArgument, Ident, ImplItem, ImplItemFn, ItemImpl,
    LitStr, PatType, Result, ReturnType, Token, Type, TypePath, parse::Parse, parse_macro_input,
    punctuated::Punctuated,
};

#[proc_macro_derive(Leaf, attributes(leaf))]
pub fn derive_leaf(input: TokenStream) -> TokenStream {
    match expand_leaf(parse_macro_input!(input as DeriveInput)) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

#[proc_macro_attribute]
pub fn procedures(attr: TokenStream, item: TokenStream) -> TokenStream {
    match expand_procedures(
        parse_macro_input!(attr as ProceduresAttributes),
        parse_macro_input!(item as ItemImpl),
    ) {
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

fn expand_procedures(
    attr: ProceduresAttributes,
    mut item: ItemImpl,
) -> Result<proc_macro2::TokenStream> {
    let self_ty = item.self_ty.clone();
    let impl_generics = item.generics.clone();
    let (impl_generics_tokens, ty_generics, where_clause) = impl_generics.split_for_impl();
    let error_ty = attr.error.ok_or_else(|| {
        Error::new_spanned(
            &item.self_ty,
            "missing #[procedures(error = MyError)] attribute",
        )
    })?;

    let mut dispatch_arms = Vec::new();

    for impl_item in &mut item.items {
        let ImplItem::Fn(method) = impl_item else {
            continue;
        };
        if !take_call_attr(&mut method.attrs) {
            continue;
        }

        dispatch_arms.push(expand_call_arm(method)?);
    }

    if dispatch_arms.is_empty() {
        return Err(Error::new_spanned(
            &item.self_ty,
            "#[procedures] requires at least one #[call] method",
        ));
    }

    let suffix_literals = dispatch_arms
        .iter()
        .map(|arm| arm.suffix_literal.clone())
        .collect::<Vec<_>>();
    let procedure_matches = dispatch_arms.iter().map(|arm| {
        let suffix = &arm.suffix_literal;
        quote! { #suffix => <Self as ::unshell::protocol::tree::CallProcedures>::procedure_id(#suffix), }
    });
    let dispatch_checks = dispatch_arms.iter().map(|arm| arm.dispatch_tokens.clone());

    Ok(quote! {
        #item

        impl #impl_generics_tokens ::unshell::protocol::tree::CallProcedures for #self_ty #where_clause {
            type Error = #error_ty;

            fn procedure_suffixes() -> &'static [&'static str] {
                &[#(#suffix_literals),*]
            }

            fn dispatch_call(
                &mut self,
                call: ::unshell::protocol::tree::IncomingCall,
            ) -> ::core::result::Result<
                ::unshell::protocol::tree::CallReply,
                ::unshell::protocol::tree::DispatchError<Self::Error>,
            > {
                #(#dispatch_checks)*
                unreachable!("protocol runtime validated local procedure dispatch")
            }
        }

        impl #impl_generics_tokens #self_ty #ty_generics #where_clause {
            /// Returns the canonical protocol leaf metadata for this type.
            pub fn protocol_leaf_spec() -> ::unshell::protocol::tree::LeafSpec {
                <Self as ::unshell::protocol::tree::CallProcedures>::leaf_spec()
            }

            /// Resolves one local procedure suffix to its full canonical `procedure_id`.
            pub fn protocol_procedure_id(
                suffix: &str,
            ) -> ::core::option::Option<::unshell::alloc::string::String> {
                match suffix {
                    #(#procedure_matches)*
                    _ => ::core::option::Option::None,
                }
            }
        }
    })
}

struct CallArm {
    suffix_literal: LitStr,
    dispatch_tokens: proc_macro2::TokenStream,
}

fn expand_call_arm(method: &ImplItemFn) -> Result<CallArm> {
    let method_name = &method.sig.ident;
    let suffix_literal = LitStr::new(&method_name.to_string(), method_name.span());
    let call_id_expr = quote! {
        <Self as ::unshell::protocol::tree::CallProcedures>::procedure_id(#suffix_literal)
            .expect("generated procedure id must exist")
    };

    let inputs = method
        .sig
        .inputs
        .iter()
        .filter(|input| !matches!(input, FnArg::Receiver(_)))
        .collect::<Vec<_>>();

    let invocation = expand_invocation(method_name, &inputs)?;
    let return_value = expand_return_conversion(&method.sig.output, quote! { __unshell_result })?;

    Ok(CallArm {
        suffix_literal: suffix_literal.clone(),
        dispatch_tokens: quote! {
            if call.message.procedure_id == #call_id_expr {
                let __unshell_result = #invocation;
                return { #return_value };
            }
        },
    })
}

fn expand_invocation(method_name: &Ident, inputs: &[&FnArg]) -> Result<proc_macro2::TokenStream> {
    if inputs.is_empty() {
        return Ok(quote! { self.#method_name() });
    }

    if inputs.len() == 1 {
        let FnArg::Typed(PatType { ty, .. }) = inputs[0] else {
            return Err(Error::new_spanned(
                inputs[0],
                "unsupported receiver in procedure signature",
            ));
        };

        if let Some(inner) = extract_call_inner_type(ty) {
            return Ok(quote! {{
                let __unshell_input = ::unshell::protocol::tree::decode_call_input::<#inner>(
                    call.message.data.as_slice(),
                )
                .map_err(::unshell::protocol::tree::DispatchError::Decode)?;
                let __unshell_call = ::unshell::protocol::tree::Call {
                    input: __unshell_input,
                    caller_path: call.header.src_path.clone(),
                    procedure_id: call.message.procedure_id.clone(),
                    dst_leaf: call.header.dst_leaf.clone(),
                    response_hook: call
                        .message
                        .response_hook
                        .as_ref()
                        .map(|hook| ::unshell::protocol::tree::HookKey::new(
                            hook.return_path.clone(),
                            hook.hook_id,
                        )),
                };
                self.#method_name(__unshell_call)
            }});
        }

        return Ok(quote! {{
            let __unshell_input = ::unshell::protocol::tree::decode_call_input::<#ty>(
                call.message.data.as_slice(),
            )
            .map_err(::unshell::protocol::tree::DispatchError::Decode)?;
            self.#method_name(__unshell_input)
        }});
    }

    let tuple_types = inputs
        .iter()
        .map(|input| match input {
            FnArg::Typed(PatType { ty, .. }) => Ok(ty.clone()),
            other => Err(Error::new_spanned(
                other,
                "unsupported receiver in procedure signature",
            )),
        })
        .collect::<Result<Vec<_>>>()?;
    let vars = (0..tuple_types.len())
        .map(|index| format_ident!("__unshell_arg_{index}"))
        .collect::<Vec<_>>();

    Ok(quote! {{
        let (#(#vars),*) = ::unshell::protocol::tree::decode_call_input::<(#(#tuple_types),*)>(
            call.message.data.as_slice(),
        )
        .map_err(::unshell::protocol::tree::DispatchError::Decode)?;
        self.#method_name(#(#vars),*)
    }})
}

fn expand_return_conversion(
    return_type: &ReturnType,
    value: proc_macro2::TokenStream,
) -> Result<proc_macro2::TokenStream> {
    match return_type {
        ReturnType::Default => Ok(quote! {
            let _ = #value;
            ::core::result::Result::Ok(::unshell::protocol::tree::CallReply::NoReply)
        }),
        ReturnType::Type(_, ty) => normalize_output_type(ty, value),
    }
}

fn normalize_output_type(
    ty: &Type,
    value: proc_macro2::TokenStream,
) -> Result<proc_macro2::TokenStream> {
    if is_unit_type(ty) {
        return Ok(quote! {
            let _ = #value;
            ::core::result::Result::Ok(::unshell::protocol::tree::CallReply::NoReply)
        });
    }

    if let Some(inner) = extract_outer_type_argument(ty, "CallResult") {
        let inner_conversion = normalize_reply_value(inner, quote! { __unshell_value })?;
        return Ok(quote! {
            match #value {
                ::unshell::protocol::tree::CallResult::Reply(__unshell_value) => {
                    #inner_conversion
                }
                ::unshell::protocol::tree::CallResult::NoReply => {
                    ::core::result::Result::Ok(::unshell::protocol::tree::CallReply::NoReply)
                }
            }
        });
    }

    if let Some((ok_ty, _error_ty)) = extract_result_type_arguments(ty) {
        let ok_conversion = normalize_output_type(ok_ty, quote! { __unshell_value })?;
        return Ok(quote! {
            match #value {
                ::core::result::Result::Ok(__unshell_value) => { #ok_conversion }
                ::core::result::Result::Err(__unshell_error) => {
                    ::core::result::Result::Err(
                        ::unshell::protocol::tree::DispatchError::Handler(__unshell_error)
                    )
                }
            }
        });
    }

    normalize_reply_value(ty, value)
}

fn normalize_reply_value(
    _ty: &Type,
    value: proc_macro2::TokenStream,
) -> Result<proc_macro2::TokenStream> {
    Ok(quote! {
        ::core::result::Result::Ok(::unshell::protocol::tree::CallReply::Reply(
            ::unshell::protocol::tree::encode_call_reply(&#value)
                .map_err(::unshell::protocol::tree::DispatchError::Encode)?
        ))
    })
}

fn extract_call_inner_type(ty: &Type) -> Option<&Type> {
    extract_outer_type_argument(ty, "Call")
}

fn extract_outer_type_argument<'a>(ty: &'a Type, expected: &str) -> Option<&'a Type> {
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

fn extract_result_type_arguments(ty: &Type) -> Option<(&Type, &Type)> {
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

fn is_unit_type(ty: &Type) -> bool {
    matches!(ty, Type::Tuple(tuple) if tuple.elems.is_empty())
}

fn take_call_attr(attrs: &mut Vec<Attribute>) -> bool {
    let original_len = attrs.len();
    attrs.retain(|attr| !attr.path().is_ident("call"));
    original_len != attrs.len()
}

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

#[derive(Default)]
struct ProceduresAttributes {
    error: Option<Type>,
}

impl Parse for ProceduresAttributes {
    fn parse(input: syn::parse::ParseStream<'_>) -> Result<Self> {
        if input.is_empty() {
            return Ok(Self::default());
        }

        let mut parsed = Self::default();
        let assignments = Punctuated::<Assignment, Token![,]>::parse_terminated(input)?;
        for assignment in assignments {
            if assignment.name == "error" {
                if parsed.error.is_some() {
                    return Err(Error::new_spanned(
                        assignment.name,
                        "duplicate procedures error attribute",
                    ));
                }
                parsed.error = Some(assignment.value);
                continue;
            }
            return Err(Error::new_spanned(
                assignment.name,
                "unsupported #[procedures(...)] attribute",
            ));
        }
        Ok(parsed)
    }
}

struct Assignment {
    name: Ident,
    value: Type,
}

impl Parse for Assignment {
    fn parse(input: syn::parse::ParseStream<'_>) -> Result<Self> {
        Ok(Self {
            name: input.parse()?,
            value: {
                input.parse::<Token![=]>()?;
                input.parse()?
            },
        })
    }
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
