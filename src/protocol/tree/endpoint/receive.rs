//! Packet ingress and local call dispatch.

use crate::protocol::types::{ArchivedCallMessage, ArchivedDataMessage, ArchivedFaultMessage};
use crate::protocol::{
    CallMessage, ProtocolFault, decode_frame, deserialize_archived_bytes,
    introspection::INTROSPECTION_PROCEDURE_ID, validate_call, validate_header,
};

use super::super::{ActiveHook, HookKey, RouteDecision};
use super::core::{
    Endpoint, EndpointError, EndpointOutcome, Ingress, LocalEvent, ProtocolEndpoint,
};

impl ProtocolEndpoint {
    fn local_procedure_fault(
        &self,
        dst_leaf: Option<&str>,
        procedure_id: &str,
    ) -> Option<ProtocolFault> {
        match dst_leaf {
            Some(leaf_name) => match self.leaves.get(leaf_name) {
                Some(leaf) => (!leaf
                    .procedures
                    .iter()
                    .any(|procedure| procedure == procedure_id))
                .then_some(ProtocolFault::UNKNOWN_PROCEDURE),
                None => Some(ProtocolFault::UNKNOWN_LEAF),
            },
            None => (!self.endpoint_procedures.contains(procedure_id))
                .then_some(ProtocolFault::UNKNOWN_PROCEDURE),
        }
    }

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

        if let Some(fault) =
            self.local_procedure_fault(header.dst_leaf.as_deref(), &message.procedure_id)
        {
            return self.emit_fault_if_possible(key, fault);
        }

        if let Some(hook) = &message.response_hook
            && hook.return_path != self.path
        {
            // Calls targeting this endpoint may still ask another endpoint to host the response
            // hook. Only register a local active hook when the response path escapes this node.
            let Some(key) = key.clone() else {
                unreachable!("response_hook checked above");
            };
            if self
                .hooks
                .insert_active(
                    key.clone(),
                    ActiveHook {
                        peer_path: header.src_path.clone(),
                        procedure_id: message.procedure_id.clone(),
                        local_ended: false,
                        peer_ended: false,
                    },
                )
                .is_err()
            {
                return self.emit_fault_if_possible(Some(key), ProtocolFault::INTERNAL_ERROR);
            }
        }

        Ok(EndpointOutcome::Local(LocalEvent::Call { header, message }))
    }

    fn receive_call(
        &mut self,
        ingress: &Ingress,
        parsed: crate::protocol::ParsedFrame<'_>,
    ) -> Result<EndpointOutcome, EndpointError> {
        // Calls only enter from the parent side of the tree or from the endpoint itself.
        // Children can return data/faults, but they do not initiate new calls through this node.
        if !matches!(ingress, Ingress::Parent | Ingress::Local) {
            return Ok(EndpointOutcome::Dropped);
        }

        let (header, payload) = parsed.into_parts();
        let message = deserialize_archived_bytes::<ArchivedCallMessage, CallMessage>(payload)?;
        validate_call(&header, &message)?;
        self.handle_local_call(header, message)
    }

    fn receive_data(
        &mut self,
        parsed: crate::protocol::ParsedFrame<'_>,
    ) -> Result<EndpointOutcome, EndpointError> {
        let (header, payload) = parsed.into_parts();
        let message = deserialize_archived_bytes::<
            ArchivedDataMessage,
            crate::protocol::DataMessage,
        >(payload)?;
        self.handle_local_data(header, message)
    }

    fn receive_fault(
        &mut self,
        parsed: crate::protocol::ParsedFrame<'_>,
    ) -> Result<EndpointOutcome, EndpointError> {
        let (header, payload) = parsed.into_parts();
        let message = deserialize_archived_bytes::<
            ArchivedFaultMessage,
            crate::protocol::FaultMessage,
        >(payload)?;
        self.handle_local_fault(header, message)
    }

    fn forward_or_drop(
        route: RouteDecision,
        frame: crate::protocol::FrameBytes,
    ) -> EndpointOutcome {
        match route {
            RouteDecision::Child(index) => EndpointOutcome::Forward {
                route: RouteDecision::Child(index),
                frame,
            },
            RouteDecision::Parent => EndpointOutcome::Forward {
                route: RouteDecision::Parent,
                frame,
            },
            RouteDecision::Drop => EndpointOutcome::Dropped,
            RouteDecision::Local => unreachable!("local routes are handled before forwarding"),
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
            return Ok(EndpointOutcome::Dropped);
        }

        let route = self.decide_route(&header.dst_path);
        if route != RouteDecision::Local {
            return Ok(Self::forward_or_drop(route, frame));
        }

        match header.packet_type {
            crate::protocol::PacketType::Call => self.receive_call(ingress, parsed),
            crate::protocol::PacketType::Data => self.receive_data(parsed),
            crate::protocol::PacketType::Fault => self.receive_fault(parsed),
        }
    }
}
