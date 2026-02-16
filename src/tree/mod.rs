//! Tree system for hierarchical message routing between endpoints.
//!
//! The tree provides a modular IPC mechanism where components expose
//! a tree of messageable elements. Used for C2 communication and pivoting.

pub mod branch;
pub mod component;
pub mod endpoint;
pub mod log;
pub mod message;
pub mod queue;
pub mod readonly;
pub mod symbols;

pub use branch::Branch;
pub use component::ComponentRegistry;
pub use endpoint::EndpointManager;
pub use message::TreeMessage;
pub use readonly::{ReadOnly, TreeVariable};

pub use symbols::*;

use serde_json::Value;

pub trait TreeElement: Send + Sync {
    fn get_type(&self) -> Value;
    fn send_message(&mut self, target: Value, message: Value) -> Value;
}
