//! Packet builders and endpoint construction.

use alloc::{collections::BTreeSet, string::String, vec::Vec};

use crate::protocol::tree::{ActiveHook, HookKey};
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
        if let Some(hook) = &call.response_hook
            && self
                .hooks
                .insert_active(ActiveHook {
                    return_path: hook.return_path.clone(),
                    hook_id: hook.hook_id,
                    peer_path: header.dst_path.clone(),
                    procedure_id: call.procedure_id.clone(),
                    dst_leaf: header.dst_leaf.clone(),
                    local_ended: false,
                    peer_ended: false,
                })
                .is_err()
        {
            return Err(EndpointError::Validation(ValidationError::InvalidHookId));
        }
        Ok(())
    }

    #[must_use]
    pub fn new(
        path: Vec<String>,
        parent_path: Option<Vec<String>>,
        children: Vec<ChildRoute>,
        leaves: Vec<LeafSpec>,
    ) -> Self {
        let registered_children = children
            .iter()
            .filter(|child| child.state == super::core::ConnectionState::Registered)
            .map(|child| child.path.clone())
            .collect::<Vec<_>>();

        Self {
            routing: CompiledRoutes::new(&path, &registered_children, parent_path.is_some()),
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
    pub fn allocate_hook_id(&mut self) -> u64 {
        self.hooks.allocate_hook_id(&self.path)
    }

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
            route => Ok(EndpointOutcome::forward(route, encode_packet(&header, &call)?)),
        }
    }

    pub fn make_data(
        &self,
        dst_path: Vec<String>,
        hook_id: u64,
        procedure_id: impl Into<String>,
        data: Vec<u8>,
        end_hook: bool,
    ) -> Result<FrameBytes, EndpointError> {
        let (header, message) = self.prepare_data(dst_path, hook_id, procedure_id, data, end_hook)?;
        Ok(encode_packet(&header, &message)?)
    }

    pub fn send_data(
        &mut self,
        dst_path: Vec<String>,
        hook_id: u64,
        procedure_id: impl Into<String>,
        data: Vec<u8>,
        end_hook: bool,
    ) -> Result<EndpointOutcome, EndpointError> {
        let (header, message) = self.prepare_data(dst_path, hook_id, procedure_id, data, end_hook)?;

        if end_hook {
            let sender_key = self
                .hooks
                .resolve_active_key(&self.path, hook_id, &self.path)
                .unwrap_or_else(|| HookKey::new(self.path.clone(), hook_id));
            if self.hooks.mark_local_end(&sender_key) {
                self.hooks.remove_active(&sender_key);
            }
        }

        match self.decide_route(&header.dst_path) {
            RouteDecision::Local => self.handle_local_data(header, message),
            route => Ok(EndpointOutcome::forward(route, encode_packet(&header, &message)?)),
        }
    }
}
