//! Packet builders and basic endpoint configuration.
//!
//! These helpers map to `PROTOCOL.md` sections covering packet construction,
//! call headers, and hook declaration fields.

use alloc::{collections::BTreeSet, string::String, vec::Vec};

use crate::protocol::tree::ActiveHook;
use crate::protocol::{
    CallMessage, DataMessage, FrameBytes, HookTarget, PacketHeader, PacketType, ValidationError,
    encode_packet, validate_call, validate_header, validate_procedure_id,
};

use super::super::RouteDecision;
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
                })
                .is_err()
        {
            return Err(EndpointError::Validation(ValidationError::InvalidHookId));
        }
        Ok(())
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

    /// Creates a runtime endpoint with static tree topology and leaf metadata.
    ///
    /// ```
    /// use unshell::protocol::tree::{Endpoint, ProtocolEndpoint};
    ///
    /// let endpoint = ProtocolEndpoint::new(Vec::new(), None, Vec::new(), Vec::new());
    /// assert!(endpoint.path().is_empty());
    /// ```
    #[must_use]
    pub fn new(
        path: Vec<String>,
        parent_path: Option<Vec<String>>,
        children: Vec<ChildRoute>,
        leaves: Vec<LeafSpec>,
    ) -> Self {
        Self {
            path,
            parent_path,
            children,
            leaves: leaves
                .into_iter()
                .map(|leaf| (leaf.name.clone(), leaf))
                .collect(),
            endpoint_procedures: BTreeSet::new(),
            hooks: Default::default(),
        }
    }

    /// Registers an endpoint-local procedure identifier.
    pub fn add_endpoint_procedure(
        &mut self,
        procedure_id: impl Into<String>,
    ) -> Result<(), EndpointError> {
        let procedure_id = procedure_id.into();
        validate_procedure_id(&procedure_id)?;
        self.endpoint_procedures.insert(procedure_id);
        Ok(())
    }

    /// Allocates a locally unique hook id.
    #[must_use]
    pub fn allocate_hook_id(&mut self) -> u64 {
        self.hooks.allocate_hook_id(&self.path)
    }

    /// Builds an outbound `Call` packet and pre-registers active hook state when requested.
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

    /// Routes one locally originated `Call` without an encode/decode roundtrip.
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

    /// Builds an outbound `Data` packet for an existing hook.
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

    /// Routes one locally originated `Data` packet without an encode/decode roundtrip.
    pub fn send_data(
        &mut self,
        dst_path: Vec<String>,
        hook_id: u64,
        procedure_id: impl Into<String>,
        data: Vec<u8>,
        end_hook: bool,
    ) -> Result<EndpointOutcome, EndpointError> {
        let (header, message) = self.prepare_data(dst_path, hook_id, procedure_id, data, end_hook)?;

        match self.decide_route(&header.dst_path) {
            RouteDecision::Local => self.handle_local_data(header, message),
            route => Ok(EndpointOutcome::forward(route, encode_packet(&header, &message)?)),
        }
    }
}
