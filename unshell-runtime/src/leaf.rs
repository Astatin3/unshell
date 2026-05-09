//! Leaf-facing runtime types.

use crate::alloc::boxed::Box;
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

/// One leaf handler registered with a runtime-local dispatch key.
///
/// The id is the packet `dst_leaf` name used by [`unshell_protocol::tree::LocalEvent`]
/// call headers. The runtime keeps this intentionally small: it only finds the
/// target callback and records requested [`crate::context::LeafAction`] values.
pub struct RegisteredLeaf<Error> {
    id: LeafId,
    capabilities: LeafCapabilities,
    handler: Box<dyn Leaf<Error = Error>>,
}

impl<Error> RegisteredLeaf<Error> {
    /// Creates a registered leaf from an explicit dispatch id and handler.
    #[must_use]
    pub fn new<L>(id: LeafId, handler: L) -> Self
    where
        L: Leaf<Error = Error> + 'static,
    {
        let capabilities = handler.capabilities().clone();
        Self {
            id,
            capabilities,
            handler: Box::new(handler),
        }
    }

    /// Returns the dispatch id used for local packet matching.
    #[must_use]
    pub const fn id(&self) -> &LeafId {
        &self.id
    }

    /// Returns the capabilities cached at registration time.
    #[must_use]
    pub const fn capabilities(&self) -> &LeafCapabilities {
        &self.capabilities
    }

    /// Returns immutable access to the hosted leaf.
    #[must_use]
    pub fn handler(&self) -> &dyn Leaf<Error = Error> {
        self.handler.as_ref()
    }

    /// Returns mutable access to the hosted leaf.
    #[must_use]
    pub fn handler_mut(&mut self) -> &mut dyn Leaf<Error = Error> {
        self.handler.as_mut()
    }

    /// Returns all fields needed to invoke a leaf without cloning metadata.
    pub(crate) fn dispatch_parts_mut(
        &mut self,
    ) -> (&LeafId, &LeafCapabilities, &mut dyn Leaf<Error = Error>) {
        (&self.id, &self.capabilities, self.handler.as_mut())
    }
}

impl<Error> core::fmt::Debug for RegisteredLeaf<Error> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RegisteredLeaf")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}
