//! Packet ingress and local call dispatch.
//!
//! This file implements the transport-facing packet entry point and maps it to
//! the `Call`, `Data`, and `Fault` sections of `PROTOCOL.md`.

use alloc::vec;

use crate::protocol::{
    CallMessage, DataMessage, PacketType, ProtocolFault, decode_frame,
    introspection::INTROSPECTION_PROCEDURE_ID, validate_call, validate_header,
};

use super::super::{HookKey, PendingHook, RouteDecision};
use super::core::{
    Endpoint, EndpointError, EndpointOutcome, Ingress, LocalEvent, ProtocolEndpoint,
};

impl ProtocolEndpoint {
    /// Handles a locally delivered `Call` packet after routing selected `Local`.
    pub(crate) fn handle_local_call(
        &mut self,
        header: crate::protocol::PacketHeader,
        message: CallMessage,
    ) -> Result<EndpointOutcome, EndpointError> {
        let key = message
            .response_hook
            .as_ref()
            .map(|hook| HookKey::new(hook.return_path.clone(), hook.hook_id));

        if let Some(hook) = &message.response_hook
            && hook.return_path != self.path
            && self
                .hooks
                .insert_pending(PendingHook {
                    caller_src_path: header.src_path.clone(),
                    return_path: hook.return_path.clone(),
                    hook_id: hook.hook_id,
                    procedure_id: message.procedure_id.clone(),
                    dst_leaf: header.dst_leaf.clone(),
                })
                .is_err()
        {
            return self.emit_fault_if_possible(key, ProtocolFault::INTERNAL_ERROR);
        }

        if message.procedure_id == INTROSPECTION_PROCEDURE_ID {
            return self.handle_introspection(&header, key);
        }

        let supported = match &header.dst_leaf {
            Some(leaf_name) => self
                .leaves
                .get(leaf_name)
                .map(|leaf| {
                    leaf.procedures
                        .iter()
                        .any(|procedure| procedure == &message.procedure_id)
                })
                .unwrap_or(false),
            None => self.endpoint_procedures.contains(&message.procedure_id),
        };

        if !supported {
            let fault = if header
                .dst_leaf
                .as_ref()
                .is_some_and(|name| !self.leaves.contains_key(name))
            {
                ProtocolFault::UNKNOWN_LEAF
            } else {
                ProtocolFault::UNKNOWN_PROCEDURE
            };
            return self.emit_fault_if_possible(key, fault);
        }

        if let Some(key) = &key {
            self.hooks.activate_pending(key, header.src_path.clone());
        }

        match header
            .dst_leaf
            .as_ref()
            .and_then(|name| self.leaves.get(name))
        {
            Some(leaf) if leaf.behavior == super::core::LeafBehavior::Echo && key.is_some() => {
                let hook = message.response_hook.expect("synchronized");
                let response = DataMessage {
                    procedure_id: message.procedure_id.clone(),
                    data: message.data,
                    end_hook: true,
                };
                let response_header = crate::protocol::PacketHeader {
                    packet_type: PacketType::Data,
                    src_path: self.path.clone(),
                    dst_path: hook.return_path.clone(),
                    dst_leaf: None,
                    hook_id: Some(hook.hook_id),
                };
                let route = self.decide_route(&hook.return_path);
                self.hooks
                    .remove_active(&HookKey::new(hook.return_path.clone(), hook.hook_id));

                match route {
                    RouteDecision::Local => Ok(EndpointOutcome {
                        events: vec![LocalEvent::Data {
                            header: response_header,
                            message: response,
                        }],
                        ..EndpointOutcome::default()
                    }),
                    _ => {
                        let frame = crate::protocol::encode_packet(&response_header, &response)?;
                        Ok(EndpointOutcome {
                            forwards: vec![(route, frame)],
                            ..EndpointOutcome::default()
                        })
                    }
                }
            }
            _ => Ok(EndpointOutcome {
                events: vec![LocalEvent::Call { header, message }],
                ..EndpointOutcome::default()
            }),
        }
    }
}

impl Endpoint for ProtocolEndpoint {
    fn path(&self) -> &[alloc::string::String] {
        &self.path
    }

    fn receive(
        &mut self,
        ingress: &Ingress,
        frame: crate::protocol::FrameBytes,
    ) -> Result<EndpointOutcome, EndpointError> {
        let parsed = decode_frame(&frame)?;
        let header = parsed.header();
        validate_header(header)?;

        if !self.valid_source_for_ingress(ingress, &header.src_path) {
            return Ok(EndpointOutcome {
                dropped: true,
                ..EndpointOutcome::default()
            });
        }

        match header.packet_type {
            PacketType::Call => {
                let message = parsed.deserialize_call()?;
                if !matches!(ingress, Ingress::Parent | Ingress::Local) {
                    return Ok(EndpointOutcome {
                        dropped: true,
                        ..EndpointOutcome::default()
                    });
                }

                validate_call(header, &message)?;
                match self.decide_route(&header.dst_path) {
                    RouteDecision::Child(index) => Ok(EndpointOutcome {
                        forwards: vec![(RouteDecision::Child(index), frame)],
                        ..EndpointOutcome::default()
                    }),
                    RouteDecision::Parent => Ok(EndpointOutcome {
                        forwards: vec![(RouteDecision::Parent, frame)],
                        ..EndpointOutcome::default()
                    }),
                    RouteDecision::Drop => Ok(EndpointOutcome {
                        dropped: true,
                        ..EndpointOutcome::default()
                    }),
                    RouteDecision::Local => self.handle_local_call(header.clone(), message),
                }
            }
            PacketType::Data => {
                let message = parsed.deserialize_data()?;
                match self.decide_route(&header.dst_path) {
                    RouteDecision::Local => self.handle_local_data(header.clone(), message),
                    RouteDecision::Child(index) => Ok(EndpointOutcome {
                        forwards: vec![(RouteDecision::Child(index), frame)],
                        ..EndpointOutcome::default()
                    }),
                    RouteDecision::Parent => Ok(EndpointOutcome {
                        forwards: vec![(RouteDecision::Parent, frame)],
                        ..EndpointOutcome::default()
                    }),
                    RouteDecision::Drop => Ok(EndpointOutcome {
                        dropped: true,
                        ..EndpointOutcome::default()
                    }),
                }
            }
            PacketType::Fault => {
                let message = parsed.deserialize_fault()?;
                match self.decide_route(&header.dst_path) {
                    RouteDecision::Local => self.handle_local_fault(header.clone(), message),
                    RouteDecision::Child(index) => Ok(EndpointOutcome {
                        forwards: vec![(RouteDecision::Child(index), frame)],
                        ..EndpointOutcome::default()
                    }),
                    RouteDecision::Parent => Ok(EndpointOutcome {
                        forwards: vec![(RouteDecision::Parent, frame)],
                        ..EndpointOutcome::default()
                    }),
                    RouteDecision::Drop => Ok(EndpointOutcome {
                        dropped: true,
                        ..EndpointOutcome::default()
                    }),
                }
            }
        }
    }
}
