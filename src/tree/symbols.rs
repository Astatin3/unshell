//! Symbol constants for the tree system.
//!
//! This module provides string constants used throughout the tree system.
//! These symbols are often obfuscated at compile-time to avoid static analysis.
//!
//! # Categories
//!
//! - **Type symbols**: Define element types (Endpoint, Queue, Connection)
//! - **Command symbols**: Message commands (Get, Poll, GetLength)
//! - **Error symbols**: Error messages (UnsupportedMethod, InvalidCommand)
//! - **Key symbols**: JSON keys (method, params, config, status)
//! - **Method symbols**: RPC methods (connect, disconnect, send, recv)
//! - **State symbols**: Common state values (config, protocols, unknown)
//!
//! # Usage
//!
//! ```rust
//! use crate::tree::symbols::{TYPE_ENDPOINT, CMD_GET_CHILDREN, ERR_UNSUPPORTED_METHOD};
//!
//! // These are used internally for tree communication
//! let endpoint_type = TYPE_ENDPOINT;
//! let get_children_cmd = CMD_GET_CHILDREN;
//! let error_msg = ERR_UNSUPPORTED_METHOD;
//! ```
//!
//! # Obfuscation
//!
//! When the `obfuscate` feature is enabled, these strings are encrypted
//! at compile-time using AES, making static analysis more difficult.

use crate::obfuscate::sym;

pub const LOGGER: &'static str = sym!("Logger");

pub const TYPE_TREE: &'static str = sym!("Tree");
pub const TYPE_QUEUE: &'static str = sym!("Queue");
pub const TYPE_ENDPOINT: &'static str = sym!("Endpoint");
pub const TYPE_CONNECTIONS: &'static str = sym!("Connections");
pub const TYPE_CONNECTION: &'static str = sym!("Connection");

pub const CMD_GET: &'static str = sym!("Get");
pub const CMD_POLL: &'static str = sym!("Poll");
pub const CMD_GET_LENGTH: &'static str = sym!("GetLength");
pub const CMD_GET_CHILDREN: &'static str = sym!("GetChildren");

pub const ERR_UNSUPPORTED_METHOD: &'static str = sym!("UnsupportedMethod");
pub const ERR_INVALID_COMMAND: &'static str = sym!("InvalidCommand");
pub const ERR_INVALID_CHILD: &'static str = sym!("InvalidChild");
pub const ERR_INVALID_TARGET: &'static str = sym!("InvalidTarget");
pub const ERR_CHILD_NOT_FOUND: &'static str = sym!("ChildNotFound");
pub const ERR_INVALID_PATH: &'static str = sym!("InvalidPath");
pub const ERR_MISSING_ARGS: &'static str = sym!("MissingArgs");
pub const ERR_INVALID_STATE: &'static str = sym!("InvalidState");
pub const ERR_READONLY: &'static str = sym!("ReadOnly");

pub const TYPE_TCP_CLIENT: &'static str = sym!("TCPClient");
pub const TYPE_TCP_SERVER: &'static str = sym!("TCPServer");

pub const KEY_METHOD: &'static str = sym!("method");
pub const KEY_PARAMS: &'static str = sym!("params");
pub const KEY_SUCCESS: &'static str = sym!("success");
pub const KEY_ERROR: &'static str = sym!("error");
pub const KEY_CONFIG: &'static str = sym!("config");
pub const KEY_STATUS: &'static str = sym!("status");
pub const KEY_ADDRESS: &'static str = sym!("address");
pub const KEY_PORT: &'static str = sym!("port");
pub const KEY_DATA: &'static str = sym!("data");
pub const KEY_SIZE: &'static str = sym!("size");
pub const KEY_CLIENT_ID: &'static str = sym!("client_id");
pub const KEY_BYTES_SENT: &'static str = sym!("bytes_sent");
pub const KEY_BYTES_RECEIVED: &'static str = sym!("bytes_received");
pub const KEY_CONNECTED: &'static str = sym!("connected");
pub const KEY_REMOTE_ADDRESS: &'static str = sym!("remote_address");
pub const KEY_LOCAL_ADDRESS: &'static str = sym!("local_address");
pub const KEY_PROTOCOLS: &'static str = sym!("protocols");
pub const KEY_TYPE: &'static str = sym!("type");
pub const KEY_NAME: &'static str = sym!("name");
pub const KEY_ID: &'static str = sym!("id");
pub const KEY_PEER: &'static str = sym!("peer");
pub const KEY_LISTENING: &'static str = sym!("listening");
pub const KEY_BIND_ADDRESS: &'static str = sym!("bind_address");
pub const KEY_ACTIVE_CONNECTIONS: &'static str = sym!("active_connections");
pub const KEY_TOTAL_CONNECTIONS: &'static str = sym!("total_connections");
pub const KEY_CLIENTS: &'static str = sym!("clients");

pub const METHOD_CONNECT: &'static str = sym!("connect");
pub const METHOD_DISCONNECT: &'static str = sym!("disconnect");
pub const METHOD_SEND: &'static str = sym!("send");
pub const METHOD_RECV: &'static str = sym!("recv");
pub const METHOD_STATUS: &'static str = sym!("status");
pub const METHOD_SET_PROTOCOLS: &'static str = sym!("set_protocols");
pub const METHOD_LISTEN: &'static str = sym!("listen");
pub const METHOD_START: &'static str = sym!("start");
pub const METHOD_STOP: &'static str = sym!("stop");
pub const METHOD_ACCEPT: &'static str = sym!("accept");
pub const METHOD_LIST_CLIENTS: &'static str = sym!("list_clients");

pub const CMD_CONNECT: &'static str = sym!("Connect");
pub const CMD_DISCONNECT: &'static str = sym!("Disconnect");
pub const CMD_STATUS: &'static str = sym!("Status");
pub const CMD_LISTEN: &'static str = sym!("Listen");
pub const CMD_START: &'static str = sym!("Start");
pub const CMD_STOP: &'static str = sym!("Stop");

pub const STR_STATE: &'static str = sym!("state");
pub const STR_CONFIG: &'static str = sym!("config");
pub const STR_PROTOCOLS: &'static str = sym!("protocols");
pub const STR_UNKNOWN: &'static str = sym!("unknown");
pub const STR_0_0_0_0: &'static str = sym!("0.0.0.0");

pub const ERR_MISSING_METHOD: &'static str = sym!("missing method");
pub const ERR_MISSING_DATA: &'static str = sym!("missing data");
pub const ERR_MISSING_CLIENT_ID: &'static str = sym!("missing client_id");
pub const ERR_MISSING_PROTOCOLS: &'static str = sym!("missing protocols");
pub const ERR_INVALID_CONFIG: &'static str = sym!("Invalid config: {}");
pub const ERR_INVALID_PROTOCOLS: &'static str = sym!("Invalid protocols: {}");
pub const ERR_UNKNOWN_METHOD: &'static str = sym!("unknown method: {}");
