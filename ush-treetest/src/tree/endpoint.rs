//! # Tree Endpoint
//! 
//! This module defines the Endpoint trait that all tree leaves must implement.
//! Endpoints handle requests and stream data for specific paths in the tree.

use crate::protocol::{TreeRequest, TreeResponse, EndpointType};
use std::string::String;

/// Endpoint trait - implemented by all leaf handlers in the tree
/// 
/// This trait is object-safe and must be Send + Sync to allow sharing across threads.
pub trait Endpoint: Send + Sync {
    /// Handle a request and return a response
    fn handle_request(&mut self, request: &TreeRequest, src_path: &str) -> Result<TreeResponse, String>;
    
    /// Called when a stream is opened to this endpoint
    /// 
    /// Returns the stream ID if successful, None if rejected
    fn on_stream_open(&mut self, stream_id: u16, src_path: &str) -> Option<u16>;
    
    /// Called when data is received on a stream
    /// 
    /// Returns true if data was handled successfully
    fn on_stream_data(&mut self, stream_id: u16, data: &[u8]) -> bool;
    
    /// Called when a stream is closed
    fn on_stream_close(&mut self, stream_id: u16);
    
    /// Get the type of this endpoint
    fn endpoint_type(&self) -> EndpointType;
    
    /// Get the name of this endpoint
    fn name(&self) -> &str;
}

/// Stream - represents an active stream between endpoints
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Stream {
    /// Unique identifier for this stream
    pub stream_id: u16,
    /// Destination path for the stream
    pub dst_path: String,
    /// Source path for the stream
    pub src_path: String,
}