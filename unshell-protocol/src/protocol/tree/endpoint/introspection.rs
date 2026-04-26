//! Introspection response generation.

use alloc::{string::String, vec::Vec};
use rkyv::{rancor::Error as RkyvError, to_bytes};

use crate::protocol::{
    DataMessage, EndpointIntrospection, FrameError, LeafIntrospection, LeafIntrospectionSummary,
    PacketHeader, PacketType, ProtocolFault, encode_packet,
};

use super::super::HookKey;
use super::core::{EndpointError, EndpointOutcome, ProtocolEndpoint};

impl ProtocolEndpoint {
    pub(crate) fn handle_introspection(
        &mut self,
        header: &PacketHeader,
        key: Option<HookKey>,
    ) -> Result<EndpointOutcome, EndpointError> {
        let Some(key) = key else {
            return Ok(EndpointOutcome::Dropped);
        };

        let response_payload = if let Some(leaf_name) = &header.dst_leaf {
            let Some(leaf) = self.leaves.get(leaf_name) else {
                return self.emit_fault_if_possible(Some(key), ProtocolFault::UNKNOWN_LEAF);
            };
            self.serialize_introspection(&LeafIntrospection {
                leaf_name: leaf_name.clone(),
                procedures: leaf.procedures.clone(),
            })?
        } else {
            self.serialize_introspection(&EndpointIntrospection {
                sub_endpoints: self.direct_registered_child_names(),
                leaves: self
                    .leaves
                    .values()
                    .map(|leaf| LeafIntrospectionSummary {
                        leaf_name: leaf.name.clone(),
                        procedures: leaf.procedures.clone(),
                    })
                    .collect(),
            })?
        };

        let response_header = PacketHeader {
            packet_type: PacketType::Data,
            src_path: self.path.clone(),
            dst_path: key.return_path.clone(),
            dst_leaf: None,
            hook_id: Some(key.hook_id),
        };
        let response = DataMessage {
            procedure_id: String::new(),
            data: response_payload,
            end_hook: true,
        };

        // Introspection always completes in a single response frame.
        if self.hooks.mark_local_end(&key) {
            self.hooks.remove_active(&key);
        }

        match self.decide_route(&key.return_path) {
            super::super::RouteDecision::Local => {
                Ok(EndpointOutcome::Local(super::core::LocalEvent::Data {
                    header: response_header,
                    message: response,
                    hook_key: key,
                }))
            }
            route => Ok(EndpointOutcome::Forward {
                route,
                frame: encode_packet(&response_header, &response)?,
            }),
        }
    }

    fn direct_registered_child_names(&self) -> Vec<String> {
        self.children
            .iter()
            .filter(|child| child.registered)
            // Child routes store absolute endpoint paths. Index the first segment below the
            // current endpoint so discovery only reports direct descendants.
            .filter_map(|child| child.path.get(self.path.len()).cloned())
            .collect()
    }

    fn serialize_introspection<T>(&self, value: &T) -> Result<Vec<u8>, EndpointError>
    where
        T: for<'a> rkyv::Serialize<
                rkyv::api::high::HighSerializer<
                    rkyv::util::AlignedVec,
                    rkyv::ser::allocator::ArenaHandle<'a>,
                    RkyvError,
                >,
            >,
    {
        to_bytes::<RkyvError>(value)
            .map_err(|error| EndpointError::Frame(FrameError::Serialize(error)))
            .map(|bytes| bytes.to_vec())
    }
}
