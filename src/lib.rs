//! Unshell - A modular, pluggable framework for endpoint agents.
//!
//! This library provides a tree-based hierarchical message routing system
//! for building modular, cross-platform endpoint agents. It follows the
//! philosophy that everything should be replaceable at runtime.
//!
//! # Architecture
//!
//! The core architecture consists of:
//! - **Tree Elements**: Hierarchical nodes that can send/receive messages
//! - **Components**: Extensible modules that can be registered at runtime
//! - **Protocols**: Swappable encoding/transport layers
//! - **Endpoints**: Root containers for tree elements
//!
//! # Core Concepts
//!
//! ## TreeElement Trait
//!
//! The foundation of the system. Any struct implementing `TreeElement` can
//! participate in the tree hierarchy and handle messages:
//!
//! ```rust
//! use serde_json::{json, Value};
//! use unshell::tree::TreeElement;
//!
//! struct MyElement;
//!
//! impl TreeElement for MyElement {
//!     fn get_type(&self) -> Value {
//!         json!("MyElement")
//!     }
//!
//!     fn send_message(&mut self, target: Value, message: Value) -> Value {
//!         // Handle messages here
//!         json!({"status": "ok"})
//!     }
//! }
//! ```
//!
//! ## Component Trait
//!
//! Components extend the tree with dynamic, configurable modules:
//!
//! ```rust
//! use serde_json::Value;
//! use unshell::tree::component::Component;
//!
//! struct MyComponent;
//!
//! impl Component for MyComponent {
//!     fn name(&self) -> &str { "my_component" }
//!     fn status(&self) -> Value { json!({"active": true}) }
//!     fn init(&mut self, config: Value) -> Result<(), String> { Ok(()) }
//!     fn shutdown(&mut self) -> Result<(), String> { Ok(()) }
//! }
//! ```
//!
//! # Module Organization
//!
//! - `tree`: Core tree system (routing, components, messages)
//! - `config`: Configuration structures and parsing
//! - `logger`: Logging infrastructure
//! - `obfuscate`: Compile-time string obfuscation
//!
//! # Usage
//!
//! ```rust
//! use unshell::tree::{EndpointManager, TreeElement};
//! use serde_json::json;
//!
//! // Create an endpoint
//! let mut endpoint = EndpointManager::new("my-endpoint");
//!
//! // Send messages to tree elements
//! let response = endpoint.branch.send_message(json!("id"), json!(null));
//! println!("Endpoint ID: {}", response);
//! ```

#![no_main]

pub mod config;
mod error;
pub mod logger;
pub mod tree;

mod announcement;

pub use error::{ModuleError, Result};

pub use announcement::Announcement;

// Re-exports
pub use serde_json::{json, Value};
pub use ush_obfuscate as obfuscate;
