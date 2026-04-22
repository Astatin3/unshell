//! # Proxy Endpoint
//! 
//! This module provides a proxy endpoint that routes to child nodes for ls/info operations.

#[allow(unused_imports)]
use crate::protocol::{TreeRequest, TreeResponse, EndpointType};
#[allow(unused_imports)]
use crate::tree::Endpoint;
use std::string::String;

/// A proxy endpoint that routes to children for ls/info operations.
///
/// This endpoint is used at the root ("/") to allow traversal to child endpoints
/// like /shell and /tty when there's no direct handler at the root.
#[derive(Debug)]
pub struct ProxyEndpoint {
    name: String,
    #[allow(dead_code)]
    tree: Option<Box<crate::tree::Tree>>,
}

impl ProxyEndpoint {
    /// Create a new proxy endpoint.
    ///
    /// # Arguments
    /// * `name` - The name of this endpoint (typically "proxy")
    /// * `tree` - Optional tree to proxy to
    ///
    /// # Example
    /// ```
    /// let proxy = ProxyEndpoint::new("proxy", None);
    /// ```
    #[allow(dead_code)]
    pub fn new(name: &str, _tree: Option<Box<crate::tree::Tree>>) -> Self {
        Self {
            name: name.to_string(),
            tree: None,
        }
    }

    /// Create a proxy endpoint with an empty tree.
    pub fn new_empty(name: &str) -> Self {
        Self {
            name: name.to_string(),
            tree: None,
        }
    }
}

impl crate::tree::Endpoint for ProxyEndpoint {
    fn handle_request(&mut self, request: &TreeRequest, _src_path: &str) -> Result<TreeResponse, String> {
        match request {
            TreeRequest::ListNodes { .. } => {
                if let Some(ref tree) = self.tree {
                    let names = tree.list_nodes("/").unwrap_or_default();
                    Ok(TreeResponse::NodeList { names })
                } else {
                    Ok(TreeResponse::NodeList { names: vec![] })
                }
            }
            TreeRequest::ListEndpoints { .. } => {
                if let Some(ref tree) = self.tree {
                    let endpoints = tree.list_endpoints("/").unwrap_or_default();
                    Ok(TreeResponse::EndpointList { endpoints })
                } else {
                    Ok(TreeResponse::EndpointList { endpoints: vec![] })
                }
            }
            TreeRequest::ListLeaves { .. } => {
                if let Some(ref tree) = self.tree {
                    let leaves = tree.list_leaves();
                    Ok(TreeResponse::LeafList { leaves })
                } else {
                    Ok(TreeResponse::LeafList { leaves: vec![] })
                }
            }
            TreeRequest::GetInfo { path } => {
                if let Some(ref tree) = self.tree {
                    let info = tree.get_info(path)?;
                    Ok(TreeResponse::NodeInfo { info })
                } else {
                    Err("no tree available".to_string())
                }
            }
            _ => Err("unsupported request on proxy".to_string()),
        }
    }

    fn on_stream_open(&mut self, stream_id: u16, _src_path: &str) -> Option<u16> {
        Some(stream_id)
    }

    fn on_stream_data(&mut self, _stream_id: u16, _data: &[u8]) -> bool {
        false
    }

    fn on_stream_close(&mut self, _stream_id: u16) {}

    fn endpoint_type(&self) -> EndpointType {
        EndpointType::Proxy
    }

    fn name(&self) -> &str {
        &self.name
    }
}