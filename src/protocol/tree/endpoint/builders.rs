//! Packet builders and endpoint construction.

use alloc::{collections::BTreeSet, string::String, vec::Vec};

use crate::protocol::tree::{HookKey, PendingHook};
use crate::protocol::{
    CallMessage, DataMessage, FrameBytes, HookTarget, PacketHeader, PacketType, ValidationError,
    encode_packet, validate_call, validate_header, validate_procedure_id,
};

use super::super::{CompiledRoutes, RouteDecision};
use super::core::{ChildRoute, EndpointError, EndpointOutcome, ProtocolEndpoint};
use crate::protocol::tree::LeafSpec;

impl ProtocolEndpoint {
    fn prepare_call(
        &self,
        dst_path: Vec<String>,
        dst_leaf: Option<String>,
        procedure_id: impl Into<String>,
        response_hook_id: Option<u64>,
        data: Vec<u8>,
    ) -> Result<(PacketHeader, CallMessage), EndpointError> {
        let procedure_id = procedure_id.into();
        validate_procedure_id(&procedure_id)?;

        let response_hook = response_hook_id.map(|hook_id| HookTarget {
            hook_id,
            return_path: self.path.clone(),
        });
        let header = PacketHeader {
            packet_type: PacketType::Call,
            src_path: self.path.clone(),
            dst_path,
            dst_leaf,
            hook_id: None,
        };
        let call = CallMessage {
            procedure_id,
            data,
            response_hook,
        };

        validate_header(&header)?;
        validate_call(&header, &call)?;
        Ok((header, call))
    }

    fn prepare_data(
        &self,
        dst_path: Vec<String>,
        hook_id: u64,
        procedure_id: impl Into<String>,
        data: Vec<u8>,
        end_hook: bool,
    ) -> Result<(PacketHeader, DataMessage), EndpointError> {
        let procedure_id = procedure_id.into();
        validate_procedure_id(&procedure_id)?;

        let header = PacketHeader {
            packet_type: PacketType::Data,
            src_path: self.path.clone(),
            dst_path,
            dst_leaf: None,
            hook_id: Some(hook_id),
        };
        let message = DataMessage {
            procedure_id,
            data,
            end_hook,
        };

        validate_header(&header)?;
        Ok((header, message))
    }

    fn register_outbound_call_hook(
        &mut self,
        header: &PacketHeader,
        call: &CallMessage,
    ) -> Result<(), EndpointError> {
        // Outbound calls reserve their response hook before the frame is emitted so
        // the endpoint can attribute returned Fault packets even before the callee
        // accepts the call. The hook only becomes active once valid hook traffic
        // comes back from the expected peer.
        if let Some(hook) = &call.response_hook
            && let key = HookKey::new(hook.return_path.clone(), hook.hook_id)
            && self
                .hooks
                .insert_pending(
                    key,
                    PendingHook {
                        caller_src_path: header.dst_path.clone(),
                        procedure_id: call.procedure_id.clone(),
                        local_ended: false,
                    },
                )
                .is_err()
        {
            return Err(EndpointError::Validation(ValidationError::InvalidHookId));
        }
        Ok(())
    }

    #[must_use]
    /// Creates an endpoint with compiled routing tables for its current topology.
    pub fn new(
        path: Vec<String>,
        parent_path: Option<Vec<String>>,
        children: Vec<ChildRoute>,
        leaves: Vec<LeafSpec>,
    ) -> Self {
        let registered_child_paths = children
            .iter()
            .filter(|child| child.registered)
            .map(|child| child.path.clone())
            .collect::<Vec<_>>();

        Self {
            routing: CompiledRoutes::new(&path, &registered_child_paths, parent_path.is_some()),
            path,
            children,
            leaves: leaves
                .into_iter()
                .map(|leaf| (leaf.name.clone(), leaf))
                .collect(),
            endpoint_procedures: BTreeSet::new(),
            hooks: Default::default(),
        }
    }

    /// Registers a procedure that is handled directly by the endpoint.
    pub fn add_endpoint_procedure(
        &mut self,
        procedure_id: impl Into<String>,
    ) -> Result<(), EndpointError> {
        let procedure_id = procedure_id.into();
        validate_procedure_id(&procedure_id)?;
        self.endpoint_procedures.insert(procedure_id);
        Ok(())
    }

    #[must_use]
    /// Allocates a hook id scoped to this endpoint path.
    pub fn allocate_hook_id(&mut self) -> u64 {
        self.hooks.allocate_hook_id(&self.path)
    }

    /// Encodes a call frame without routing it through the local endpoint.
    pub fn make_call(
        &mut self,
        dst_path: Vec<String>,
        dst_leaf: Option<String>,
        procedure_id: impl Into<String>,
        response_hook_id: Option<u64>,
        data: Vec<u8>,
    ) -> Result<FrameBytes, EndpointError> {
        let (header, call) =
            self.prepare_call(dst_path, dst_leaf, procedure_id, response_hook_id, data)?;
        self.register_outbound_call_hook(&header, &call)?;
        Ok(encode_packet(&header, &call)?)
    }

    /// Builds and immediately routes a call, producing either a forward or a local event.
    pub fn send_call(
        &mut self,
        dst_path: Vec<String>,
        dst_leaf: Option<String>,
        procedure_id: impl Into<String>,
        response_hook_id: Option<u64>,
        data: Vec<u8>,
    ) -> Result<EndpointOutcome, EndpointError> {
        let (header, call) =
            self.prepare_call(dst_path, dst_leaf, procedure_id, response_hook_id, data)?;
        self.register_outbound_call_hook(&header, &call)?;

        match self.decide_route(&header.dst_path) {
            RouteDecision::Local => self.handle_local_call(header, call),
            RouteDecision::Drop => {
                if let Some(hook) = &call.response_hook {
                    self.hooks
                        .remove_pending(&HookKey::new(hook.return_path.clone(), hook.hook_id));
                }
                Ok(EndpointOutcome::Dropped)
            }
            route => Ok(EndpointOutcome::Forward {
                route,
                frame: encode_packet(&header, &call)?,
            }),
        }
    }

    /// Encodes a data frame without routing it through the local endpoint.
    pub fn make_data(
        &self,
        dst_path: Vec<String>,
        hook_id: u64,
        procedure_id: impl Into<String>,
        data: Vec<u8>,
        end_hook: bool,
    ) -> Result<FrameBytes, EndpointError> {
        let (header, message) =
            self.prepare_data(dst_path, hook_id, procedure_id, data, end_hook)?;
        Ok(encode_packet(&header, &message)?)
    }

    /// Builds and immediately routes a data packet, updating local hook state for end-of-stream.
    pub fn send_data(
        &mut self,
        dst_path: Vec<String>,
        hook_id: u64,
        procedure_id: impl Into<String>,
        data: Vec<u8>,
        end_hook: bool,
    ) -> Result<EndpointOutcome, EndpointError> {
        if let Some(active_key) = self
            .hooks
            .resolve_active_key(&dst_path, hook_id, &self.path)
            && self
                .hooks
                .active(&active_key)
                .is_some_and(|active| active.local_ended)
        {
            return Err(EndpointError::Validation(ValidationError::HookInvariant(
                "local side already closed this hook",
            )));
        }

        let local_end_dst_path = dst_path.clone();
        let host_key = HookKey::new(self.path.clone(), hook_id);
        let (header, message) =
            self.prepare_data(dst_path, hook_id, procedure_id, data, end_hook)?;

        if end_hook {
            // Locally-originated streams may not have been resolved against a peer yet,
            // so fall back to the endpoint's own hook key shape when closing them.
            let local_hook_key = self
                .hooks
                .resolve_active_key(&local_end_dst_path, hook_id, &self.path)
                .unwrap_or_else(|| host_key.clone());
            if self.hooks.pending(&host_key).is_some() {
                self.hooks.mark_pending_local_end(&host_key);
            } else if self.hooks.mark_local_end(&local_hook_key) {
                self.hooks.remove_active(&local_hook_key);
            }
        }

        match self.decide_route(&header.dst_path) {
            RouteDecision::Local => self.handle_local_data(header, message),
            RouteDecision::Drop => Ok(EndpointOutcome::Dropped),
            route => Ok(EndpointOutcome::Forward {
                route,
                frame: encode_packet(&header, &message)?,
            }),
        }
    }
}
