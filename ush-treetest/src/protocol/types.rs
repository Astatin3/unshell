//! # Protocol Types
//!
//! This module defines the core types for the UnShell protocol.
//! Uses rkyv for zero-copy serialization.

use rkyv::{Archive, Serialize, Deserialize};
use std::string::String;
use std::vec::Vec;

const BUFFER_SIZE: usize = 4096;

/// Frame type enum - distinguishes between different frame kinds
#[derive(Archive, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FrameType {
    Request = 0x01,
    Response = 0x02,
    StreamOpen = 0x03,
    StreamData = 0x04,
    StreamClose = 0x05,
    Handshake = 0x10,
    HandshakeAck = 0x11,
}

impl FrameType {
    #[allow(dead_code)]
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0x01 => Some(Self::Request),
            0x02 => Some(Self::Response),
            0x03 => Some(Self::StreamOpen),
            0x04 => Some(Self::StreamData),
            0x05 => Some(Self::StreamClose),
            0x10 => Some(Self::Handshake),
            0x11 => Some(Self::HandshakeAck),
            _ => None,
        }
    }
}

/// Frame header - the metadata sent before each payload
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct FrameHeader {
    pub frame_type: FrameType,
    pub dst_path: Option<String>,
    pub src_path: String,
    pub request_id: Option<u64>,
    pub stream_id: Option<u16>,
}

impl FrameHeader {
    pub fn to_bytes(&self) -> Vec<u8> {
        rkyv::to_bytes::<FrameHeader, BUFFER_SIZE>(self).unwrap().into_vec()
    }
    
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        unsafe { rkyv::from_bytes_unchecked(bytes) }.map_err(|e| e.to_string())
    }
}

/// Tree request - operations on the tree
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub enum TreeRequest {
    ListNodes {},
    ListEndpoints {},
    ListLeaves {},
    GetInfo { path: String },
    Exec { cmd: String },
    StreamOpen { path: String },
    Resize { rows: u16, cols: u16 },
}

impl TreeRequest {
    pub fn to_bytes(&self) -> Vec<u8> {
        rkyv::to_bytes::<TreeRequest, BUFFER_SIZE>(self).unwrap().into_vec()
    }
    
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        unsafe { rkyv::from_bytes_unchecked(bytes) }.map_err(|e| e.to_string())
    }
}

/// Tree response - results from tree operations
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub enum TreeResponse {
    NodeList { names: Vec<String> },
    EndpointList { endpoints: Vec<EndpointInfo> },
    LeafList { leaves: Vec<String> },
    NodeInfo { info: NodeInfo },
    ExecOutput { exit_code: i32, stdout: Vec<u8>, stderr: Vec<u8> },
    StreamOpened { stream_id: u16 },
}

impl TreeResponse {
    pub fn to_bytes(&self) -> Vec<u8> {
        rkyv::to_bytes::<TreeResponse, BUFFER_SIZE>(self).unwrap().into_vec()
    }
    
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        unsafe { rkyv::from_bytes_unchecked(bytes) }.map_err(|e| e.to_string())
    }
}

/// Information about an endpoint
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct EndpointInfo {
    pub name: String,
    pub path: String,
    pub endpoint_type: EndpointType,
}

/// Type of endpoint
#[derive(Archive, Serialize, Deserialize, Debug, Clone, Copy)]
#[repr(u8)]
pub enum EndpointType {
    Leaf = 0x01,
    Proxy = 0x02,
    Stream = 0x03,
}

/// Information about a node in the tree
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct NodeInfo {
    pub path: String,
    pub is_leaf: bool,
    pub has_children: bool,
    pub endpoints: Vec<String>,
}

/// Handshake message - sent when connecting
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct Handshake {
    pub registered_paths: Vec<String>,
}

impl Handshake {
    pub fn to_bytes(&self) -> Vec<u8> {
        rkyv::to_bytes::<Handshake, BUFFER_SIZE>(self).unwrap().into_vec()
    }
    
    #[allow(dead_code)]
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        unsafe { rkyv::from_bytes_unchecked(bytes) }.map_err(|e| e.to_string())
    }
}

/// Handshake acknowledgement - router's response to handshake
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct HandshakeAck {
    pub accepted: bool,
    pub assigned_base_path: String,
}

impl HandshakeAck {
    pub fn to_bytes(&self) -> Vec<u8> {
        rkyv::to_bytes::<HandshakeAck, BUFFER_SIZE>(self).unwrap().into_vec()
    }
    
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        unsafe { rkyv::from_bytes_unchecked(bytes) }.map_err(|e| e.to_string())
    }
}