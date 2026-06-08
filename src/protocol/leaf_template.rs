/// Declares a generated leaf wrapper using a small template-like syntax.
///
/// The macro deliberately requires callers to name every generated session field. It
/// does not infer identifiers, inspect struct fields, or hide behavior inside a proc
/// macro. All real dispatch and retry behavior lives in normal Rust helpers.
///
/// The procedure list is handled by small internal `@...` rules instead of by
/// separate full macro expansions. That keeps the generated shape easy to audit
/// while still allowing empty `procedures {}` leaves to avoid allocating a
/// `LeafOutbox`.
#[macro_export]
macro_rules! unshell_leaf {
    (
        $vis:vis leaf $Leaf:ident for $State:ty {
            id: $id:expr,
            meta: $meta:expr,
            sessions { $( $session_field:ident : $Session:ty ),* $(,)? }
            procedures { $( $procedure_field:ident : $Procedure:ty ),* $(,)? }
        }
    ) => {
        $vis struct $Leaf {
            state: $State,
            outbox: $crate::unshell_leaf!(@outbox_type $( $procedure_field : $Procedure ),*),
            $(
                $session_field: $crate::protocol::SessionFamily<$Session>,
            )*
        }

        impl $Leaf {
            /// Creates the generated leaf wrapper around user-owned state.
            pub fn new(state: $State) -> Self {
                Self {
                    state,
                    outbox: $crate::unshell_leaf!(@outbox_new $( $procedure_field : $Procedure ),*),
                    $(
                        $session_field: $crate::protocol::SessionFamily::new(),
                    )*
                }
            }

            /// Returns immutable access to the user-owned leaf state.
            pub fn state(&self) -> &$State {
                &self.state
            }

            /// Returns mutable access to the user-owned leaf state.
            pub fn state_mut(&mut self) -> &mut $State {
                &mut self.state
            }

            /// Returns the number of active session entries across all families.
            pub fn active_session_count(&self) -> usize {
                0usize $(+ self.$session_field.entries.len())*
            }

            /// Returns queued packets owned by this generated leaf.
            pub fn pending_packet_count(&self) -> usize {
                $crate::unshell_leaf!(
                    @outbox_len
                    &self.outbox;
                    $( $procedure_field : $Procedure ),*
                ) $(+ self.$session_field.pending_packet_count())*
            }

            fn __unshell_packet_is_owned(packet: &$crate::protocol::Packet) -> bool {
                let _ = packet;

                false
                $(
                    || packet.procedure_id
                        == <$Session as $crate::protocol::Session<$State>>::PROCEDURE_ID
                )*
                $(
                    || packet.procedure_id
                        == <$Procedure as $crate::protocol::Procedure<$State>>::PROCEDURE_ID
                )*
            }

            fn __unshell_update_inner(
                &mut self,
                endpoint: &mut $crate::protocol::Endpoint,
            ) {
                let leaf_id = $id;
                let _ = leaf_id;

                $crate::unshell_leaf!(
                    @flush_outbox
                    endpoint,
                    &mut self.outbox;
                    $( $procedure_field : $Procedure ),*
                );

                let Some(local_id) = endpoint.path.last().copied() else {
                    return;
                };

                let mut packets = $crate::alloc::vec::Vec::new();
                endpoint.take_inbound_matching(
                    local_id,
                    Self::__unshell_packet_is_owned,
                    |packet| packets.push(packet),
                );

                for packet in packets {
                    self.__unshell_dispatch_packet(endpoint, packet);
                }

                $(
                    $crate::protocol::update_session_family::<$State, $Session>(
                        endpoint,
                        &mut self.state,
                        &mut self.$session_field,
                    );
                )*

                $crate::unshell_leaf!(
                    @flush_outbox
                    endpoint,
                    &mut self.outbox;
                    $( $procedure_field : $Procedure ),*
                );
            }

            fn __unshell_dispatch_packet(
                &mut self,
                endpoint: &mut $crate::protocol::Endpoint,
                packet: $crate::protocol::Packet,
            ) {
                let leaf_id = $id;
                let _ = leaf_id;

                $(
                    if packet.procedure_id
                        == <$Session as $crate::protocol::Session<$State>>::PROCEDURE_ID
                    {
                        $crate::protocol::dispatch_session::<$State, $Session>(
                            endpoint,
                            &mut self.state,
                            &mut self.$session_field,
                            packet,
                        );
                        return;
                    }
                )*

                $(
                    if packet.procedure_id
                        == <$Procedure as $crate::protocol::Procedure<$State>>::PROCEDURE_ID
                    {
                        let _ = stringify!($procedure_field);
                        $crate::protocol::dispatch_procedure::<$State, $Procedure>(
                            &mut self.state,
                            endpoint,
                            packet,
                            &mut self.outbox,
                        );
                        return;
                    }
                )*

                let _ = endpoint;
                let _ = packet;
            }

            #[cfg(feature = "interface_ratatui")]
            fn __unshell_write_interface_sessions(
                &self,
                ctx: &mut $crate::interface::InterfaceContext<'_>,
            ) {
                let leaf_id = $id;

                $(
                    for entry in &self.$session_field.entries {
                        let mut bytes = $crate::alloc::vec::Vec::new();
                        let wrote_state =
                            <$Session as $crate::protocol::Session<$State>>::serialize_interface_state(
                                &entry.state,
                                &mut bytes,
                            );

                        if wrote_state {
                            let namespace = $crate::interface::session_namespace(
                                leaf_id,
                                <$Session as $crate::protocol::Session<$State>>::PROCEDURE_ID,
                            );
                            let key = $crate::interface::hook_key(entry.hook_id);

                            ctx.database.put(&namespace, &key, &bytes);
                        }
                    }
                )*
            }

            #[cfg(feature = "interface_ratatui")]
            fn __unshell_historical_session_count(
                &self,
                ctx: &mut $crate::interface::InterfaceContext<'_>,
            ) -> usize {
                let leaf_id = $id;
                let mut count = 0usize;

                $(
                    let namespace = $crate::interface::session_namespace(
                        leaf_id,
                        <$Session as $crate::protocol::Session<$State>>::PROCEDURE_ID,
                    );

                    for (_, bytes) in ctx.database.scan(&namespace) {
                        if <$Session as $crate::protocol::Session<$State>>::deserialize_interface_state(
                            &bytes,
                        )
                        .is_some()
                        {
                            count += 1;
                        }
                    }
                )*

                count
            }

            #[cfg(feature = "interface_ratatui")]
            fn __unshell_render_active_interface_sessions(
                &self,
                ctx: &mut $crate::interface::InterfaceContext<'_>,
                frame: &mut $crate::protocol::ratatui::Frame<'_>,
                area: $crate::protocol::ratatui::layout::Rect,
            ) {
                $(
                    for entry in &self.$session_field.entries {
                        <$Session as $crate::protocol::Session<$State>>::render_interface_ratatui(
                            &self.state,
                            &entry.state,
                            ctx,
                            frame,
                            area,
                        );
                    }
                )*
            }

            #[cfg(feature = "interface_ratatui")]
            fn __unshell_render_historical_interface_sessions(
                &self,
                ctx: &mut $crate::interface::InterfaceContext<'_>,
                frame: &mut $crate::protocol::ratatui::Frame<'_>,
                area: $crate::protocol::ratatui::layout::Rect,
            ) {
                let leaf_id = $id;

                $(
                    let namespace = $crate::interface::session_namespace(
                        leaf_id,
                        <$Session as $crate::protocol::Session<$State>>::PROCEDURE_ID,
                    );

                    for (_, bytes) in ctx.database.scan(&namespace) {
                        if let Some(session) =
                            <$Session as $crate::protocol::Session<$State>>::deserialize_interface_state(
                                &bytes,
                            )
                        {
                            <$Session as $crate::protocol::Session<$State>>::render_interface_ratatui(
                                &self.state,
                                &session,
                                ctx,
                                frame,
                                area,
                            );
                        }
                    }
                )*
            }

        }

        impl $crate::protocol::Leaf for $Leaf {
            fn get_id(&self) -> u32 {
                $id
            }

            #[inline(never)]
            fn update(&mut self, endpoint: &mut $crate::protocol::Endpoint) {
                self.__unshell_update_inner(endpoint);
            }

            #[cfg(feature = "interface")]
            fn get_meta(&self) -> $crate::protocol::LeafMeta {
                $meta
            }

            #[cfg(feature = "interface_ratatui")]
            fn update_interface_ratatui(
                &mut self,
                endpoint: &mut $crate::protocol::Endpoint,
                ctx: &mut $crate::interface::InterfaceContext<'_>,
                frame: &mut $crate::protocol::ratatui::Frame<'_>,
                area: $crate::protocol::ratatui::layout::Rect,
            ) {
                self.__unshell_update_inner(endpoint);
                self.__unshell_write_interface_sessions(ctx);

                let historical_session_count = self.__unshell_historical_session_count(ctx);
                let meta = $meta;
                let areas = ctx.ratatui.render_leaf_chrome(
                    &meta,
                    self.active_session_count(),
                    historical_session_count,
                    frame,
                    area,
                );

                self.__unshell_render_active_interface_sessions(
                    ctx,
                    frame,
                    areas.active_sessions,
                );
                self.__unshell_render_historical_interface_sessions(
                    ctx,
                    frame,
                    areas.historical_sessions,
                );
            }
        }
    };

    // Select the leaf-level outbox type. Empty procedure lists use `()` so
    // session-only leaves carry no retry queue, while non-empty lists share the
    // normal procedure response queue.
    (@outbox_type) => {
        ()
    };

    (@outbox_type $first_field:ident : $FirstProcedure:ty $(, $procedure_field:ident : $Procedure:ty )* $(,)?) => {
        $crate::protocol::LeafOutbox
    };

    // Construct the procedure outbox selected by `@outbox_type`.
    (@outbox_new) => {
        ()
    };

    (@outbox_new $first_field:ident : $FirstProcedure:ty $(, $procedure_field:ident : $Procedure:ty )* $(,)?) => {
        $crate::protocol::LeafOutbox::new()
    };

    // Count queued procedure packets without forcing session-only leaves to own a queue.
    (@outbox_len $outbox:expr;) => {
        0usize
    };

    (@outbox_len $outbox:expr; $first_field:ident : $FirstProcedure:ty $(, $procedure_field:ident : $Procedure:ty )* $(,)?) => {
        $outbox.len()
    };

    // Flush queued procedure responses when the leaf declares at least one procedure.
    (@flush_outbox $endpoint:expr, $outbox:expr;) => {};

    (@flush_outbox $endpoint:expr, $outbox:expr; $first_field:ident : $FirstProcedure:ty $(, $procedure_field:ident : $Procedure:ty )* $(,)?) => {{
        let _ = stringify!($first_field);
        $(
            let _ = stringify!($procedure_field);
        )*

        $crate::protocol::flush_leaf_outbox($endpoint, $outbox);
    }};

}
