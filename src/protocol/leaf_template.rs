/// Declares a generated leaf wrapper using a small template-like syntax.
///
/// The macro deliberately requires callers to name every generated session field. It
/// does not infer identifiers, inspect struct fields, or hide behavior inside a proc
/// macro. All real dispatch and retry behavior lives in normal Rust helpers.
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
            outbox: $crate::protocol::LeafOutbox,
            $(
                $session_field: $crate::protocol::SessionFamily<
                    <$Session as $crate::protocol::Session<$State>>::State,
                >,
            )*
        }

        impl $Leaf {
            /// Creates the generated leaf wrapper around user-owned state.
            pub fn new(state: $State) -> Self {
                Self {
                    state,
                    outbox: $crate::protocol::LeafOutbox::new(),
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
                let mut count = self.outbox.len();
                $(
                    count += self.$session_field.pending_packet_count();
                )*
                count
            }

            fn __unshell_packet_is_owned(packet: &$crate::protocol::Packet) -> bool {
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
                mut interface: Option<&mut $crate::interface::InterfaceStore>,
            ) {
                let leaf_id = $id;
                self.__unshell_flush_all(endpoint, $crate::interface::borrow_store(&mut interface));

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
                    self.__unshell_dispatch_packet(
                        endpoint,
                        packet,
                        $crate::interface::borrow_store(&mut interface),
                    );
                }

                $(
                    $crate::protocol::update_session_family::<$State, $Session>(
                        leaf_id,
                        &mut self.state,
                        &mut self.$session_field,
                        $crate::interface::borrow_store(&mut interface),
                    );
                )*

                self.__unshell_flush_all(endpoint, $crate::interface::borrow_store(&mut interface));
            }

            fn __unshell_dispatch_packet(
                &mut self,
                endpoint: &mut $crate::protocol::Endpoint,
                packet: $crate::protocol::Packet,
                mut interface: Option<&mut $crate::interface::InterfaceStore>,
            ) {
                let leaf_id = $id;

                $(
                    if packet.procedure_id
                        == <$Session as $crate::protocol::Session<$State>>::PROCEDURE_ID
                    {
                        $crate::protocol::dispatch_session::<$State, $Session>(
                            leaf_id,
                            &mut self.state,
                            &mut self.$session_field,
                            packet,
                            &mut self.outbox,
                            $crate::interface::borrow_store(&mut interface),
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
                            leaf_id,
                            &mut self.state,
                            endpoint,
                            packet,
                            &mut self.outbox,
                            $crate::interface::borrow_store(&mut interface),
                        );
                        return;
                    }
                )*

                let _ = endpoint;
                let _ = packet;
            }

            fn __unshell_flush_all(
                &mut self,
                endpoint: &mut $crate::protocol::Endpoint,
                mut interface: Option<&mut $crate::interface::InterfaceStore>,
            ) {
                let leaf_id = $id;

                $crate::protocol::flush_leaf_outbox(
                    endpoint,
                    leaf_id,
                    &mut self.outbox,
                    $crate::interface::borrow_store(&mut interface),
                );

                $(
                    $crate::protocol::flush_session_family::<$State, $Session>(
                        endpoint,
                        leaf_id,
                        &mut self.$session_field,
                        $crate::interface::borrow_store(&mut interface),
                    );
                )*
            }
        }

        impl $crate::protocol::Leaf for $Leaf {
            fn get_id(&self) -> u32 {
                $id
            }

            fn update(&mut self, endpoint: &mut $crate::protocol::Endpoint) {
                self.__unshell_update_inner(endpoint, None);
            }

            #[cfg(feature = "interface")]
            fn update_interface(
                &mut self,
                endpoint: &mut $crate::protocol::Endpoint,
                interface: &mut $crate::interface::InterfaceStore,
            ) {
                self.__unshell_update_inner(endpoint, Some(interface));
            }

            #[cfg(feature = "interface")]
            fn get_meta(&self) -> $crate::protocol::LeafMeta {
                $meta
            }

            #[cfg(feature = "interface_ratatui")]
            fn render_ratatui(
                &mut self,
                frame: &mut $crate::protocol::ratatui::Frame<'_>,
                area: $crate::protocol::ratatui::layout::Rect,
                interface: &mut $crate::interface::InterfaceStore,
            ) {
                let leaf_id = $id;

                $(
                    for entry in &mut self.$session_field.entries {
                        let view = interface.session_view_mut(
                            leaf_id,
                            <$Session as $crate::protocol::Session<$State>>::PROCEDURE_ID,
                            entry.hook_id,
                        );
                        <$Session as $crate::protocol::Session<$State>>::render_ratatui(
                            &self.state,
                            &entry.state,
                            view,
                            frame,
                            area,
                        );
                    }
                )*

                $(
                    {
                        let _ = stringify!($procedure_field);
                        let view = interface.procedure_view_mut(
                            leaf_id,
                            <$Procedure as $crate::protocol::Procedure<$State>>::PROCEDURE_ID,
                        );
                        <$Procedure as $crate::protocol::Procedure<$State>>::render_ratatui(
                            &self.state,
                            view,
                            frame,
                            area,
                        );
                    }
                )*
            }
        }
    };
}
