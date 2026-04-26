use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Error, FnArg, Ident, ImplItem, ImplItemFn, ItemImpl, LitStr, PatType, Result, ReturnType,
    Token, Type, parse::Parse, punctuated::Punctuated,
};

use crate::utils::{
    extract_outer_type_argument, extract_result_type_arguments, is_unit_type, take_call_attr,
};

#[derive(Default)]
pub(crate) struct ProceduresAttributes {
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

struct CallArm {
    suffix_literal: LitStr,
    dispatch_tokens: TokenStream,
}

#[derive(Clone, Copy)]
enum EndpointArgKind {
    Shared,
    Mutable,
}

pub(crate) fn expand_procedures(
    attr: ProceduresAttributes,
    mut item: ItemImpl,
) -> Result<TokenStream> {
    let self_ty = item.self_ty.clone();
    let impl_generics = item.generics.clone();
    let (impl_generics_tokens, _ty_generics, where_clause) = impl_generics.split_for_impl();
    let error_ty = attr.error.ok_or_else(|| {
        Error::new_spanned(
            &item.self_ty,
            "missing #[procedures(error = MyError)] attribute",
        )
    })?;

    let mut dispatch_arms = Vec::new();
    let mut seen_suffixes = std::collections::BTreeSet::new();

    for impl_item in &mut item.items {
        let ImplItem::Fn(method) = impl_item else {
            continue;
        };
        let has_call_attr = method.attrs.iter().any(|attr| attr.path().is_ident("call"));
        if !has_call_attr {
            continue;
        }

        let arm = expand_call_arm(method)?;
        take_call_attr(&mut method.attrs);
        if !seen_suffixes.insert(arm.suffix_literal.value()) {
            return Err(Error::new_spanned(
                method,
                "duplicate #[call] procedure suffix in this impl block",
            ));
        }
        dispatch_arms.push(arm);
    }

    if dispatch_arms.is_empty() {
        return Err(Error::new_spanned(
            &item.self_ty,
            "#[procedures] requires at least one #[call] method",
        ));
    }

    let dispatch_checks = dispatch_arms.iter().map(|arm| arm.dispatch_tokens.clone());

    Ok(quote! {
        #item

        impl #impl_generics_tokens ::unshell::protocol::tree::CallProcedures for #self_ty #where_clause {
            type Error = #error_ty;

            fn dispatch_call(
                &mut self,
                endpoint: &mut ::unshell::protocol::tree::ProtocolEndpoint,
                call: ::unshell::protocol::tree::IncomingCall,
            ) -> ::core::result::Result<
                ::unshell::protocol::tree::CallReply,
                ::unshell::protocol::tree::DispatchError<Self::Error>,
            > {
                #(#dispatch_checks)*
                unreachable!("protocol runtime validated local procedure dispatch")
            }
        }

    })
}

fn expand_call_arm(method: &ImplItemFn) -> Result<CallArm> {
    let method_name = &method.sig.ident;
    let suffix_literal = call_suffix_literal(method)?;
    let call_id_expr = quote! {
        {
            let mut __unshell_id = <Self as ::unshell::protocol::tree::ProtocolLeaf>::leaf_name();
            __unshell_id.push('.');
            __unshell_id.push_str(#suffix_literal);
            __unshell_id
        }
    };

    let inputs = method
        .sig
        .inputs
        .iter()
        .filter(|input| !matches!(input, FnArg::Receiver(_)))
        .collect::<Vec<_>>();

    let (endpoint_arg, inputs) = split_endpoint_arg(&inputs)?;

    let invocation = expand_invocation(method_name, endpoint_arg, &inputs)?;
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

fn expand_invocation(
    method_name: &Ident,
    endpoint_arg: Option<EndpointArgKind>,
    inputs: &[&FnArg],
) -> Result<TokenStream> {
    let endpoint_prefix = endpoint_arg.map(endpoint_arg_tokens);
    if inputs.is_empty() {
        return Ok(if let Some(prefix) = endpoint_prefix {
            quote! { self.#method_name(#prefix) }
        } else {
            quote! { self.#method_name() }
        });
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
                // Rebuild the normalized `Call<T>` value expected by generated handlers from the
                // validated protocol envelope plus the typed payload we just decoded.
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
                self.#method_name(#endpoint_prefix __unshell_call)
            }});
        }

        return Ok(quote! {{
            let __unshell_input = ::unshell::protocol::tree::decode_call_input::<#ty>(
                call.message.data.as_slice(),
            )
            .map_err(::unshell::protocol::tree::DispatchError::Decode)?;
            self.#method_name(#endpoint_prefix __unshell_input)
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
        self.#method_name(#endpoint_prefix #(#vars),*)
    }})
}

fn split_endpoint_arg<'a>(inputs: &[&'a FnArg]) -> Result<(Option<EndpointArgKind>, Vec<&'a FnArg>)> {
    let Some(first) = inputs.first() else {
        return Ok((None, Vec::new()));
    };
    let Some(kind) = endpoint_arg_kind(first)? else {
        return Ok((None, inputs.to_vec()));
    };
    Ok((Some(kind), inputs[1..].to_vec()))
}

fn endpoint_arg_kind(arg: &FnArg) -> Result<Option<EndpointArgKind>> {
    let FnArg::Typed(PatType { ty, .. }) = arg else {
        return Ok(None);
    };
    let Type::Reference(reference) = ty.as_ref() else {
        return Ok(None);
    };
    let Type::Path(type_path) = reference.elem.as_ref() else {
        return Ok(None);
    };
    let Some(segment) = type_path.path.segments.last() else {
        return Ok(None);
    };
    if segment.ident != "ProtocolEndpoint" {
        return Ok(None);
    }
    Ok(Some(if reference.mutability.is_some() {
        EndpointArgKind::Mutable
    } else {
        EndpointArgKind::Shared
    }))
}

fn endpoint_arg_tokens(kind: EndpointArgKind) -> TokenStream {
    match kind {
        EndpointArgKind::Shared => quote! { &*endpoint, },
        EndpointArgKind::Mutable => quote! { endpoint, },
    }
}

fn expand_return_conversion(return_type: &ReturnType, value: TokenStream) -> Result<TokenStream> {
    match return_type {
        ReturnType::Default => Ok(quote! {
            let _ = #value;
            ::core::result::Result::Ok(::unshell::protocol::tree::CallReply::NoReply)
        }),
        ReturnType::Type(_, ty) => normalize_output_type(ty, value),
    }
}

fn normalize_output_type(ty: &Type, value: TokenStream) -> Result<TokenStream> {
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

fn normalize_reply_value(_ty: &Type, value: TokenStream) -> Result<TokenStream> {
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

fn call_suffix_literal(method: &ImplItemFn) -> Result<LitStr> {
    let mut suffix = None;

    for attr in &method.attrs {
        if !attr.path().is_ident("call") {
            continue;
        }

        if matches!(attr.meta, syn::Meta::Path(_)) {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("name") {
                if suffix.is_some() {
                    return Err(meta.error("duplicate call name attribute"));
                }
                suffix = Some(meta.value()?.parse()?);
                return Ok(());
            }

            Err(meta.error("unsupported #[call(...)] attribute"))
        })?;
    }

    let suffix = suffix
        .unwrap_or_else(|| LitStr::new(&method.sig.ident.to_string(), method.sig.ident.span()));
    if suffix.value().is_empty() {
        return Err(Error::new_spanned(&suffix, "call name must not be empty"));
    }
    if suffix.value().contains('.') {
        return Err(Error::new_spanned(
            &suffix,
            "call name must be one local suffix without dots",
        ));
    }
    if suffix.value().chars().any(char::is_whitespace) {
        return Err(Error::new_spanned(
            &suffix,
            "call name must not contain whitespace",
        ));
    }
    Ok(suffix)
}
