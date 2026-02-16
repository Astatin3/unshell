//! Payload module for network protocols and transports.
//!
//! This module provides protocol stacking, TCP client/server implementations,
//! and connection management for testing and payload operations.

pub mod connection;
pub mod protocols;
pub mod tcp;

pub use connection::{create_channel_pair, Connection, Connections};
pub use protocols::{
    Base64Config, HttpConfig, Protocol, ProtocolConfig, ProtocolError, ProtocolStack, TcpConfig,
    WebSocketConfig,
};
pub use tcp::{
    ConnectionStatus, ListenerStatus, TcpClient, TcpClientConfig, TcpServer, TcpServerConfig,
};
