//! Node-level runtime identity types.
//!
//! A node is the local runtime owner for protocol state, leaf bindings, and
//! transport connections. This module only models identity and lifecycle state.

pub mod packet;
pub mod runtime;
pub mod state;

pub use packet::{EndpointState, PacketProcessor};
pub use runtime::{NodeRuntime, NodeRuntimeError, TickBudget, TickOutcome};
pub use state::NodeState;

use crate::alloc::string::String;

/// Stable identifier for a runtime node.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NodeId(String);

impl NodeId {
    /// Creates a node identifier from an owned string.
    #[must_use]
    pub const fn new(value: String) -> Self {
        Self(value)
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the identifier and returns the owned string.
    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

/// Minimal runtime node descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Node {
    id: NodeId,
    state: NodeState,
}

impl Node {
    /// Creates a new node descriptor in the default [`NodeState::Created`] state.
    #[must_use]
    pub const fn new(id: NodeId) -> Self {
        Self {
            id,
            state: NodeState::Created,
        }
    }

    /// Returns the node identifier.
    #[must_use]
    pub const fn id(&self) -> &NodeId {
        &self.id
    }

    /// Returns the current node lifecycle state.
    #[must_use]
    pub const fn state(&self) -> NodeState {
        self.state
    }

    /// Updates the current node lifecycle state.
    pub const fn set_state(&mut self, state: NodeState) {
        self.state = state;
    }
}
