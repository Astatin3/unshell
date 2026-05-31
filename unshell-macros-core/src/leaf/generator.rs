use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Ident, ItemStruct, Result, Type};

use super::{
    UnshellLeafArgs,
    names::{last_type_ident, to_snake_case},
};

/// Code generator state for one `#[unshell_leaf]` expansion.
pub(crate) struct LeafGenerator {
    args: UnshellLeafArgs,
    state: ItemStruct,
}

impl LeafGenerator {
    /// Creates a generator for one parsed state struct.
    pub(crate) fn new(args: UnshellLeafArgs, state: ItemStruct) -> Self {
        Self { args, state }
    }

    /// Emits the original state struct plus the generated wrapper leaf.
    pub(crate) fn expand(self) -> Result<TokenStream> {
        let state = &self.state;
        let state_ident = &state.ident;
        let leaf_ident = &self.args.leaf;
        let leaf_id = &self.args.id;
        let vis = &state.vis;
        let generics = &state.generics;
        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
        let state_type = quote!(#state_ident #ty_generics);

        let session_stores = self.session_stores()?;
        let fields = self.store_fields(&session_stores, &state_type);
        let initializers = self.store_initializers(&session_stores);
        let packet_predicates = self.packet_predicates(&state_type);
        let dispatch_arms = self.dispatch_arms(&session_stores, &state_type);
        let session_updates = self.session_updates(&session_stores, &state_type);
        let session_flushes = self.session_flushes(&session_stores);
        let session_retains = self.session_retains(&session_stores);
        let active_count_terms = self.active_count_terms(&session_stores);
        let pending_count_terms = self.pending_count_terms(&session_stores);
        let id_checks = self.id_checks(&state_type);

        Ok(quote! {
            #state

            #vis struct #leaf_ident #generics #where_clause {
                state: #state_type,
                __unshell_procedure_outbox: ::unshell::protocol::PacketQueue,
                #(#fields,)*
            }

            impl #impl_generics #leaf_ident #ty_generics #where_clause {
                const __UNSHELL_PROCEDURE_ID_CHECKS: () = {
                    #(#id_checks)*
                };

                /// Creates the generated leaf wrapper around user-owned state.
                pub fn new(state: #state_type) -> Self {
                    Self {
                        state,
                        __unshell_procedure_outbox: ::unshell::protocol::PacketQueue::new(),
                        #(#initializers,)*
                    }
                }

                /// Returns immutable access to the user-owned leaf state.
                pub fn state(&self) -> &#state_type {
                    &self.state
                }

                /// Returns mutable access to the user-owned leaf state.
                pub fn state_mut(&mut self) -> &mut #state_type {
                    &mut self.state
                }

                /// Returns the number of active session entries across all session families.
                pub fn active_session_count(&self) -> usize {
                    0usize #(+ #active_count_terms)*
                }

                /// Returns queued inbound and outbound packets owned by this generated leaf.
                pub fn pending_packet_count(&self) -> usize {
                    let mut __unshell_count = self.__unshell_procedure_outbox.len();
                    #(#pending_count_terms)*
                    __unshell_count
                }

                fn __unshell_packet_is_owned(packet: &::unshell::protocol::Packet) -> bool {
                    false #(|| #packet_predicates)*
                }

                fn __unshell_dispatch(
                    &mut self,
                    endpoint: &mut ::unshell::protocol::Endpoint,
                    packet: ::unshell::protocol::Packet,
                ) {
                    #(#dispatch_arms)*
                }

                fn __unshell_update_sessions(&mut self) {
                    #(#session_updates)*
                }

                fn __unshell_flush_all(&mut self, endpoint: &mut ::unshell::protocol::Endpoint) {
                    ::unshell::protocol::flush_packet_queue(
                        endpoint,
                        &mut self.__unshell_procedure_outbox,
                    );
                    #(#session_flushes)*
                    #(#session_retains)*
                }

                fn __unshell_parent_reply_path(
                    endpoint: &::unshell::protocol::Endpoint,
                ) -> ::unshell::alloc::vec::Vec<u32> {
                    if endpoint.path.len() > 1 {
                        endpoint.path[..endpoint.path.len() - 1].to_vec()
                    } else {
                        endpoint.path.clone()
                    }
                }
            }

            impl #impl_generics ::unshell::protocol::Leaf for #leaf_ident #ty_generics #where_clause {
                fn get_id(&self) -> u32 {
                    #leaf_id
                }

                fn update(&mut self, endpoint: &mut ::unshell::protocol::Endpoint) {
                    self.__unshell_flush_all(endpoint);

                    let Some(__unshell_local_id) = endpoint.path.last().copied() else {
                        return;
                    };

                    let mut __unshell_packets = ::unshell::alloc::vec::Vec::new();
                    endpoint.take_inbound_matching(
                        __unshell_local_id,
                        Self::__unshell_packet_is_owned,
                        |packet| __unshell_packets.push(packet),
                    );

                    for __unshell_packet in __unshell_packets {
                        self.__unshell_dispatch(endpoint, __unshell_packet);
                    }

                    self.__unshell_update_sessions();
                    self.__unshell_flush_all(endpoint);
                }
            }
        })
    }

    /// Computes one generated store name per session type.
    fn session_stores(&self) -> Result<Vec<SessionStore>> {
        self.args
            .sessions
            .iter()
            .map(|session| {
                let suffix = last_type_ident(session)?;
                let field_suffix = to_snake_case(&suffix.to_string());
                Ok(SessionStore {
                    ty: session.clone(),
                    field: format_ident!("__unshell_{}_sessions", field_suffix),
                })
            })
            .collect()
    }

    /// Emits wrapper fields for session stores.
    fn store_fields(&self, stores: &[SessionStore], state_type: &TokenStream) -> Vec<TokenStream> {
        stores
            .iter()
            .map(|store| {
                let field = &store.field;
                let session_ty = &store.ty;
                quote! {
                    #field: ::unshell::alloc::vec::Vec<
                        ::unshell::protocol::SessionEntry<
                            <#session_ty as ::unshell::protocol::Session<#state_type>>::State
                        >
                    >
                }
            })
            .collect()
    }

    /// Emits constructor field initializers for session stores.
    fn store_initializers(&self, stores: &[SessionStore]) -> Vec<TokenStream> {
        stores
            .iter()
            .map(|store| {
                let field = &store.field;
                quote!(#field: ::unshell::alloc::vec::Vec::new())
            })
            .collect()
    }

    /// Emits boolean procedure-id ownership checks for the filtered endpoint drain.
    fn packet_predicates(&self, state_type: &TokenStream) -> Vec<TokenStream> {
        let session_checks = self.args.sessions.iter().map(|session_ty| {
            quote! {
                packet.procedure_id
                    == <#session_ty as ::unshell::protocol::Session<#state_type>>::PROCEDURE_ID
            }
        });
        let procedure_checks = self.args.procedures.iter().map(|procedure_ty| {
            quote! {
                packet.procedure_id
                    == <#procedure_ty as ::unshell::protocol::Procedure<#state_type>>::PROCEDURE_ID
            }
        });

        session_checks.chain(procedure_checks).collect()
    }

    /// Emits static dispatch branches for every session and procedure type.
    fn dispatch_arms(&self, stores: &[SessionStore], state_type: &TokenStream) -> Vec<TokenStream> {
        let mut arms = Vec::new();

        for store in stores {
            let field = &store.field;
            let session_ty = &store.ty;
            arms.push(quote! {
                if packet.procedure_id
                    == <#session_ty as ::unshell::protocol::Session<#state_type>>::PROCEDURE_ID
                {
                    if let Some(__unshell_entry) = self
                        .#field
                        .iter_mut()
                        .find(|entry| entry.hook_id == packet.hook_id)
                    {
                        __unshell_entry.inbox.push_back(packet);
                    } else {
                        let __unshell_hook_id = packet.hook_id;
                        let __unshell_packet_path = packet.path.clone();
                        let mut __unshell_init = ::unshell::protocol::SessionInit::new(
                            __unshell_hook_id,
                            __unshell_packet_path,
                        );

                        match <#session_ty as ::unshell::protocol::Session<#state_type>>::init(
                            &mut self.state,
                            packet,
                            &mut __unshell_init,
                        ) {
                            ::unshell::protocol::SessionInitResult::Created(__unshell_state) => {
                                self.#field.push(::unshell::protocol::SessionEntry::new(
                                    __unshell_hook_id,
                                    __unshell_state,
                                ));
                            }
                            ::unshell::protocol::SessionInitResult::Rejected => {}
                            ::unshell::protocol::SessionInitResult::RejectedWith(__unshell_packet) => {
                                self.__unshell_procedure_outbox.push_back(__unshell_packet);
                            }
                        }
                    }
                    return;
                }
            });
        }

        for procedure_ty in &self.args.procedures {
            arms.push(quote! {
                if packet.procedure_id
                    == <#procedure_ty as ::unshell::protocol::Procedure<#state_type>>::PROCEDURE_ID
                {
                    let mut __unshell_out = ::unshell::protocol::ProcedureOut::new(
                        packet.hook_id,
                        Self::__unshell_parent_reply_path(endpoint),
                        <#procedure_ty as ::unshell::protocol::Procedure<#state_type>>::PROCEDURE_ID,
                    );
                    <#procedure_ty as ::unshell::protocol::Procedure<#state_type>>::handle(
                        &mut self.state,
                        endpoint,
                        packet,
                        &mut __unshell_out,
                    );
                    self.__unshell_procedure_outbox.extend(__unshell_out.into_packets());
                    return;
                }
            });
        }

        arms
    }

    /// Emits the per-session update loop for every session family.
    fn session_updates(
        &self,
        stores: &[SessionStore],
        state_type: &TokenStream,
    ) -> Vec<TokenStream> {
        stores
            .iter()
            .map(|store| {
                let field = &store.field;
                let session_ty = &store.ty;
                quote! {
                    for __unshell_entry in &mut self.#field {
                        if __unshell_entry.closed {
                            continue;
                        }

                        let __unshell_reply_path =
                            <#session_ty as ::unshell::protocol::Session<#state_type>>::reply_path(
                                &__unshell_entry.state,
                            )
                            .to_vec();
                        let mut __unshell_ctx = ::unshell::protocol::SessionCtx::new(
                            __unshell_entry.hook_id,
                            __unshell_reply_path,
                            <#session_ty as ::unshell::protocol::Session<#state_type>>::PROCEDURE_ID,
                            &mut __unshell_entry.outbox,
                        );
                        let __unshell_status =
                            <#session_ty as ::unshell::protocol::Session<#state_type>>::update(
                                &mut self.state,
                                &mut __unshell_entry.state,
                                &mut __unshell_entry.inbox,
                                &mut __unshell_ctx,
                            );

                        if ::core::matches!(
                            __unshell_status,
                            ::unshell::protocol::SessionStatus::Closed
                        ) {
                            __unshell_entry.closed = true;
                        }
                    }
                }
            })
            .collect()
    }

    /// Emits retry flushing for every session outbox.
    fn session_flushes(&self, stores: &[SessionStore]) -> Vec<TokenStream> {
        stores
            .iter()
            .map(|store| {
                let field = &store.field;
                quote! {
                    for __unshell_entry in &mut self.#field {
                        ::unshell::protocol::flush_packet_queue(
                            endpoint,
                            &mut __unshell_entry.outbox,
                        );
                    }
                }
            })
            .collect()
    }

    /// Emits removal of closed sessions whose final packets have routed.
    fn session_retains(&self, stores: &[SessionStore]) -> Vec<TokenStream> {
        stores
            .iter()
            .map(|store| {
                let field = &store.field;
                quote! {
                    self.#field
                        .retain(|entry| !entry.closed || !entry.outbox.is_empty());
                }
            })
            .collect()
    }

    /// Emits additive terms for active session counts.
    fn active_count_terms(&self, stores: &[SessionStore]) -> Vec<TokenStream> {
        stores
            .iter()
            .map(|store| {
                let field = &store.field;
                quote!(self.#field.len())
            })
            .collect()
    }

    /// Emits statements that accumulate pending packet counts.
    fn pending_count_terms(&self, stores: &[SessionStore]) -> Vec<TokenStream> {
        stores
            .iter()
            .map(|store| {
                let field = &store.field;
                quote! {
                    for __unshell_entry in &self.#field {
                        __unshell_count +=
                            __unshell_entry.inbox.len() + __unshell_entry.outbox.len();
                    }
                }
            })
            .collect()
    }

    /// Emits pairwise const assertions for procedure-id uniqueness.
    fn id_checks(&self, state_type: &TokenStream) -> Vec<TokenStream> {
        let mut ids = Vec::new();
        for session_ty in &self.args.sessions {
            ids.push(
                quote!(<#session_ty as ::unshell::protocol::Session<#state_type>>::PROCEDURE_ID),
            );
        }
        for procedure_ty in &self.args.procedures {
            ids.push(
                quote!(<#procedure_ty as ::unshell::protocol::Procedure<#state_type>>::PROCEDURE_ID),
            );
        }

        let mut checks = Vec::new();
        for left in 0..ids.len() {
            for right in (left + 1)..ids.len() {
                let left_id = &ids[left];
                let right_id = &ids[right];
                checks.push(quote! {
                    assert!(
                        #left_id != #right_id,
                        "duplicate unshell procedure id in #[unshell_leaf]"
                    );
                });
            }
        }

        checks
    }
}

/// Generated storage metadata for one session family.
struct SessionStore {
    ty: Type,
    field: Ident,
}
