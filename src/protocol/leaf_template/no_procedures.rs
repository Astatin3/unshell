/// Expands the `unshell_leaf!` specialization for leaves without one-shot procedures.
///
/// This helper stays separate from the public macro because the no-procedure shape is
/// intentionally different: it does not allocate a `LeafOutbox`, so tiny leaves such as
/// the shell leaf avoid carrying unused procedure retry machinery in the optimized
/// endpoint binary.
#[doc(hidden)]
#[macro_export]
macro_rules! __unshell_leaf_no_procedures {
    (
        $vis:vis leaf $Leaf:ident for $State:ty {
            id: $id:expr,
            meta: $meta:expr,
            sessions { $( $session_field:ident : $Session:ty ),* $(,)? }
        }
    ) => {
        $vis struct $Leaf {
            state: $State,
            $(
                $session_field: $crate::protocol::SessionFamily<$Session>,
            )*
        }

        impl $Leaf {
            /// Creates the generated leaf wrapper around user-owned state.
            pub fn new(state: $State) -> Self {
                Self {
                    state,
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
                0usize $(+ self.$session_field.pending_packet_count())*
            }

            fn __unshell_packet_is_owned(packet: &$crate::protocol::Packet) -> bool {
                false
                $(
                    || packet.procedure_id
                        == <$Session as $crate::protocol::Session<$State>>::PROCEDURE_ID
                )*
            }

            fn __unshell_update_inner(
                &mut self,
                endpoint: &mut $crate::protocol::Endpoint,
            ) {
                let leaf_id = $id;
                let _ = leaf_id;

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
            }

            #[cfg(feature = "interface")]
            fn __unshell_update_interface_inner(
                &mut self,
                endpoint: &mut $crate::protocol::Endpoint,
                interface: &mut $crate::interface::InterfaceStore,
            ) {
                let leaf_id = $id;
                let _ = leaf_id;

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
                    self.__unshell_dispatch_packet_interface(endpoint, packet, interface);
                }

                $(
                    $crate::protocol::update_session_family_interface::<$State, $Session>(
                        endpoint,
                        leaf_id,
                        &mut self.state,
                        &mut self.$session_field,
                        interface,
                    );
                )*
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

                let _ = endpoint;
                let _ = packet;
            }

            #[cfg(feature = "interface")]
            fn __unshell_dispatch_packet_interface(
                &mut self,
                endpoint: &mut $crate::protocol::Endpoint,
                packet: $crate::protocol::Packet,
                interface: &mut $crate::interface::InterfaceStore,
            ) {
                let leaf_id = $id;

                $(
                    if packet.procedure_id
                        == <$Session as $crate::protocol::Session<$State>>::PROCEDURE_ID
                    {
                        $crate::protocol::dispatch_session_interface::<$State, $Session>(
                            endpoint,
                            leaf_id,
                            &mut self.state,
                            &mut self.$session_field,
                            packet,
                            interface,
                        );
                        return;
                    }
                )*

                let _ = endpoint;
                let _ = packet;
                let _ = interface;
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
            #[inline(never)]
            fn update_interface(
                &mut self,
                endpoint: &mut $crate::protocol::Endpoint,
                interface: &mut $crate::interface::InterfaceStore,
            ) {
                self.__unshell_update_interface_inner(endpoint, interface);
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
                let _ = leaf_id;

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
            }
        }
    };
}
