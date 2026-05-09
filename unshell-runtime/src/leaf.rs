//! Leaf-facing runtime types.

use crate::alloc::string::String;
use crate::alloc::vec::Vec;
use crate::context::LeafContext;
use unshell_protocol::tree::{IncomingCall, IncomingData, IncomingFault};

/// Stable identifier for a locally hosted leaf binding.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LeafId(String);

impl LeafId {
    /// Creates a leaf id from an owned string.
    #[must_use]
    pub const fn new(value: String) -> Self {
        Self(value)
    }

    /// Returns the leaf id as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Runtime permissions granted to one leaf binding.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LeafPermissions {
    /// The leaf may request new outbound calls.
    pub send_calls: bool,
    /// The leaf may request data or faults on hook streams.
    pub send_hook_data: bool,
    /// The leaf may request connection registration or removal.
    pub manage_connections: bool,
}

impl LeafPermissions {
    /// Grants no runtime-side effects.
    pub const NONE: Self = Self {
        send_calls: false,
        send_hook_data: false,
        manage_connections: false,
    };

    /// Grants the common permission set for a passive responder leaf.
    pub const REPLY_ONLY: Self = Self {
        send_calls: false,
        send_hook_data: true,
        manage_connections: false,
    };

    /// Grants all current permissions. Use sparingly.
    pub const ALL: Self = Self {
        send_calls: true,
        send_hook_data: true,
        manage_connections: true,
    };
}

/// Protocol surface and runtime permissions for one leaf.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LeafCapabilities {
    /// Canonical dotted leaf name.
    pub leaf_name: String,
    /// Canonical procedure ids supported by the leaf.
    pub procedures: Vec<String>,
    /// Runtime permissions granted to this leaf binding.
    pub permissions: LeafPermissions,
}

/// One hosted leaf implementation.
pub trait Leaf {
    /// Leaf-specific error type.
    type Error;

    /// Returns static protocol and runtime capabilities.
    fn capabilities(&self) -> &LeafCapabilities;

    /// Handles one opening call routed to this leaf.
    fn on_call(
        &mut self,
        _ctx: &mut LeafContext<'_>,
        _call: IncomingCall,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Handles hook data routed to this leaf or its session adapter.
    fn on_data(
        &mut self,
        _ctx: &mut LeafContext<'_>,
        _data: IncomingData,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Handles hook fault routed to this leaf or its session adapter.
    fn on_fault(
        &mut self,
        _ctx: &mut LeafContext<'_>,
        _fault: IncomingFault,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Gives the leaf one bounded opportunity to request local work.
    fn poll(&mut self, _ctx: &mut LeafContext<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
}
