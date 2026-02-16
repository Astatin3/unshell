//! TCP networking components for tree-based communication.
//!
//! This module provides TCP client and server components that can be
//! added to endpoints for network communication.

pub mod client;
pub mod config;
pub mod server;

pub use client::TcpClient;
pub use config::{ConnectionStatus, ListenerStatus, TcpClientConfig, TcpServerConfig};
pub use server::TcpServer;
