//! Tree system for hierarchical message routing between endpoints.
//!
//! The tree provides a modular IPC mechanism where components expose
//! a tree of messageable elements. This is the core communication
//! abstraction used throughout unshell.
//!
//! # Design Philosophy
//!
//! The tree system follows these principles:
//! - **Everything is a TreeElement**: Components, queues, variables all share the same interface
//! - **Path-based routing**: Messages target elements by path (e.g., "components/tcp-client")
//! - **Loose typing**: Actions and payloads are flexible JSON values
//! - **Namespaced actions**: Use dot notation (e.g., "rpc.call", "stream.data")
//!
//! # Core Traits
//!
//! ## TreeElement
//!
//! The fundamental trait that all tree nodes implement:
//!
//! ```rust
//! use serde_json::{json, Value};
//! use unshell::tree::TreeElement;
//!
//! struct MyNode {
//!     value: i32,
//! }
//!
//! impl TreeElement for MyNode {
//!     fn get_type(&self) -> Value {
//!         json!("MyNode")
//!     }
//!
//!     fn send_message(&mut self, target: Value, message: Value) -> Value {
//!         // Handle the message and return a response
//!         json!({"received": true})
//!     }
//! }
//! ```
//!
//! ## Component
//!
//! A trait for extensible modules that can be dynamically registered:
//!
//! ```rust
//! use serde_json::Value;
//! use unshell::tree::component::Component;
//!
//! struct MyComponent {
//!     name: String,
//! }
//!
//! impl Component for MyComponent {
//!     fn name(&self) -> &str { &self.name }
//!     fn status(&self) -> Value { json!({"running": true}) }
//!     fn init(&mut self, config: Value) -> Result<(), String> { Ok(()) }
//!     fn shutdown(&mut self) -> Result<(), String> { Ok(()) }
//! }
//! ```
//!
//! # Message Format
//!
//! Messages in the tree follow a simple RPC-like pattern:
//!
//! ```rust
//! use serde_json::json;
//!
//! // Target a specific element
//! let target = json!("components/tcp-client");
//!
//! // Send an RPC-like message
//! let message = json!({
//!     "method": "connect",
//!     "params": {"address": "127.0.0.1", "port": 8080}
//! });
//! ```
//!
//! # Example: Creating an Endpoint
//!
//! ```rust
//! use unshell::tree::{EndpointManager, TreeElement};
//! use serde_json::json;
//!
//! // Create a new endpoint with id, logs, and components
//! let mut endpoint = EndpointManager::new("my-endpoint");
//!
//! // Query children
//! let children = endpoint.branch.send_message(json!(null), json!("GetChildren"));
//!
//! // Get the endpoint ID
//! let id = endpoint.branch.send_message(json!("id"), json!(null));
//! ```
//!
//! # Modules
//!
//! - `branch`: Branch nodes that contain child elements
//! - `component`: Component registration and lifecycle management
//! - `endpoint`: Root endpoint management
//! - `log`: Log handling
//! - `message`: TreeMessage serialization format
//! - `queue`: Message queue implementation
//! - `readonly`: Read-only variable wrappers
//! - `symbols`: String constants (often obfuscated)

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
