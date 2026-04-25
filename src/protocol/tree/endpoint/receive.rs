//! Packet ingress and local call dispatch.
//!
//! This file implements the transport-facing packet entry point and maps it to
//! the `Call`, `Data`, and `Fault` sections of `PROTOCOL.md`.

use crate::protocol::{
    CallMessage, PacketType, ProtocolFault, decode_frame, introspection::INTROSPECTION_PROCEDURE_ID,
    validate_call, validate_header,
};

use super::super::{ActiveHook, HookKey, RouteDecision};
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

        if let Some(hook) = &message.response_hook
            && hook.return_path != self.path
            && self
                .hooks
                .insert_active(ActiveHook {
                    return_path: hook.return_path.clone(),
                    hook_id: hook.hook_id,
                    peer_path: header.src_path.clone(),
                    procedure_id: message.procedure_id.clone(),
                    dst_leaf: header.dst_leaf.clone(),
                    peer_finished: false,
                })
                .is_err()
        {
            return self.emit_fault_if_possible(key, ProtocolFault::INTERNAL_ERROR);
        }

        Ok(EndpointOutcome::event(LocalEvent::Call { header, message }))
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
            return Ok(EndpointOutcome::dropped());
        }

        match header.packet_type {
            PacketType::Call => {
                let message = parsed.deserialize_call()?;
                if !matches!(ingress, Ingress::Parent | Ingress::Local) {
                    return Ok(EndpointOutcome::dropped());
                }

                validate_call(header, &message)?;
                match self.decide_route(&header.dst_path) {
                    RouteDecision::Child(index) => {
                        Ok(EndpointOutcome::forward(RouteDecision::Child(index), frame))
                    }
                    RouteDecision::Parent => {
                        Ok(EndpointOutcome::forward(RouteDecision::Parent, frame))
                    }
                    RouteDecision::Drop => Ok(EndpointOutcome::dropped()),
                    RouteDecision::Local => self.handle_local_call(parsed.deserialize_header(), message),
                }
            }
            PacketType::Data => {
                match self.decide_route(&header.dst_path) {
                    RouteDecision::Local => {
                        let message = parsed.deserialize_data()?;
                        self.handle_local_data(parsed.deserialize_header(), message)
                    }
                    RouteDecision::Child(index) => {
                        Ok(EndpointOutcome::forward(RouteDecision::Child(index), frame))
                    }
                    RouteDecision::Parent => {
                        Ok(EndpointOutcome::forward(RouteDecision::Parent, frame))
                    }
                    RouteDecision::Drop => Ok(EndpointOutcome::dropped()),
                }
            }
            PacketType::Fault => {
                match self.decide_route(&header.dst_path) {
                    RouteDecision::Local => {
                        let message = parsed.deserialize_fault()?;
                        self.handle_local_fault(parsed.deserialize_header(), message)
                    }
                    RouteDecision::Child(index) => {
                        Ok(EndpointOutcome::forward(RouteDecision::Child(index), frame))
                    }
                    RouteDecision::Parent => {
                        Ok(EndpointOutcome::forward(RouteDecision::Parent, frame))
                    }
                    RouteDecision::Drop => Ok(EndpointOutcome::dropped()),
                }
            }
        }
    }
}
