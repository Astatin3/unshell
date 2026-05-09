//! Request-only context exposed to leaf callbacks.
//!
//! Leaf code never receives direct access to route tables, hook state, endpoint
//! internals, or transport handles. It can only enqueue [`LeafAction`] values.
//! The runtime validates and applies those actions later.

use crate::alloc::string::String;
use crate::alloc::vec::Vec;
use crate::connections::{ConnectionDirection, ConnectionId, Connections};
use crate::leaf::{LeafCapabilities, LeafId};
use unshell_protocol::ProtocolFault;

/// Context handed to one leaf callback.
#[derive(Debug)]
pub struct LeafContext<'a> {
    local_path: &'a [String],
    leaf_id: &'a LeafId,
    capabilities: &'a LeafCapabilities,
    connections: &'a Connections,
    actions: Vec<LeafAction>,
}

impl<'a> LeafContext<'a> {
    /// Creates a context for one leaf callback.
    #[must_use]
    pub const fn new(
        local_path: &'a [String],
        leaf_id: &'a LeafId,
        capabilities: &'a LeafCapabilities,
        connections: &'a Connections,
    ) -> Self {
        Self {
            local_path,
            leaf_id,
            capabilities,
            connections,
            actions: Vec::new(),
        }
    }

    /// Returns this endpoint's absolute path.
    #[must_use]
    pub const fn local_path(&self) -> &[String] {
        self.local_path
    }

    /// Returns the leaf currently using this context.
    #[must_use]
    pub const fn leaf_id(&self) -> &LeafId {
        self.leaf_id
    }

    /// Returns the permissions granted to this leaf.
    #[must_use]
    pub const fn capabilities(&self) -> &LeafCapabilities {
        self.capabilities
    }

    /// Returns read-only connection metadata.
    #[must_use]
    pub const fn connections(&self) -> &Connections {
        self.connections
    }

    /// Returns queued leaf actions.
    #[must_use]
    pub fn actions(&self) -> &[LeafAction] {
        &self.actions
    }

    /// Consumes the context and returns queued actions.
    #[must_use]
    pub fn into_actions(self) -> Vec<LeafAction> {
        self.actions
    }

    /// Requests an outbound call.
    pub fn call(&mut self, call: OutboundCall) -> Result<(), RequestDenied> {
        if !self.capabilities.permissions.send_calls {
            return Err(RequestDenied::MissingCapability(
                RuntimeCapability::SendCalls,
            ));
        }
        self.actions.push(LeafAction::SendCall(call));
        Ok(())
    }

    /// Requests data on an existing hook.
    pub fn hook_data(&mut self, data: OutboundHookData) -> Result<(), RequestDenied> {
        if !self.capabilities.permissions.send_hook_data {
            return Err(RequestDenied::MissingCapability(
                RuntimeCapability::SendHookData,
            ));
        }
        self.actions.push(LeafAction::SendHookData(data));
        Ok(())
    }

    /// Requests hook termination with a protocol fault.
    pub fn fail_hook(&mut self, hook_id: u64, fault: ProtocolFault) -> Result<(), RequestDenied> {
        if !self.capabilities.permissions.send_hook_data {
            return Err(RequestDenied::MissingCapability(
                RuntimeCapability::SendHookData,
            ));
        }
        self.actions.push(LeafAction::FailHook { hook_id, fault });
        Ok(())
    }

    /// Requests a connection admission or teardown action.
    pub fn connection(&mut self, request: ConnectionAction) -> Result<(), RequestDenied> {
        if !self.capabilities.permissions.manage_connections {
            return Err(RequestDenied::MissingCapability(
                RuntimeCapability::ManageConnections,
            ));
        }
        self.actions.push(LeafAction::Connection(request));
        Ok(())
    }
}

/// Runtime action requested by leaf code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LeafAction {
    /// Build and send one outbound call.
    SendCall(OutboundCall),
    /// Build and send one hook data packet.
    SendHookData(OutboundHookData),
    /// Terminate a hook with a protocol fault.
    FailHook {
        /// Hook identifier scoped by the hook host.
        hook_id: u64,
        /// Stable protocol fault code.
        fault: ProtocolFault,
    },
    /// Request a connection state change.
    Connection(ConnectionAction),
}

/// Outbound call request before packet construction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutboundCall {
    /// Destination endpoint path.
    pub dst_path: Vec<String>,
    /// Optional destination leaf name.
    pub dst_leaf: Option<String>,
    /// Canonical procedure id.
    pub procedure_id: String,
    /// Opaque request payload.
    pub payload: Vec<u8>,
    /// Whether the runtime should allocate a response hook.
    pub expects_response: bool,
}

/// Hook data request before packet construction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutboundHookData {
    /// Destination endpoint path for the hook packet.
    pub dst_path: Vec<String>,
    /// Hook identifier scoped by the receiving endpoint.
    pub hook_id: u64,
    /// Canonical procedure id associated with the hook stream.
    pub procedure_id: String,
    /// Opaque payload bytes.
    pub payload: Vec<u8>,
    /// Whether this packet closes the local side of the hook.
    pub end_hook: bool,
}

/// Requested connection state change.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConnectionAction {
    /// Register an existing connection as a direct parent or child.
    Register {
        /// Runtime transport connection id.
        connection: ConnectionId,
        /// Requested tree direction.
        direction: ConnectionDirection,
        /// Peer path to register.
        peer_path: Vec<String>,
    },
    /// Remove a connection from runtime routing.
    Unregister {
        /// Runtime transport connection id.
        connection: ConnectionId,
    },
}

/// Capability checked by [`LeafContext`] helpers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeCapability {
    /// Permission to request outbound calls.
    SendCalls,
    /// Permission to request hook data or hook faults.
    SendHookData,
    /// Permission to request connection state changes.
    ManageConnections,
}

/// Rejection reason for a leaf action request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestDenied {
    /// The leaf does not have the required capability.
    MissingCapability(RuntimeCapability),
}
