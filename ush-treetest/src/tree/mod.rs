//! # Tree Module
//! 
//! This module implements the tree-based routing for the unshell protocol.
//! The tree structure maintains endpoints at paths and handles routing of
//! requests and streams to appropriate handlers.

pub mod endpoint;
pub use endpoint::{Endpoint, Stream};

use crate::protocol::{EndpointInfo, FrameHeader, NodeInfo};
use std::collections::BTreeMap;
use std::string::String;
use std::vec::Vec;
use std::boxed::Box;
use std::result::Result;
use std::sync::{Arc, Mutex};
use std::fmt;

/// A node in the tree - contains an optional endpoint and child nodes.
pub struct Node {
    endpoint: Option<Arc<Mutex<Box<dyn Endpoint + 'static>>>>,
    children: BTreeMap<String, Node>,
    streams: BTreeMap<u16, Stream>,
    next_stream_id: u16,
    path: String,
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Node")
            .field("path", &self.path)
            .field("children", &self.children.keys().cloned().collect::<Vec<_>>())
            .finish()
    }
}

impl Node {
    /// Create a new node with the given path
    pub fn new(path: &str) -> Self {
        Self {
            endpoint: None,
            children: BTreeMap::new(),
            streams: BTreeMap::new(),
            next_stream_id: 1,
            path: path.to_string(),
        }
    }
    
    /// Set the endpoint for this node
    /// 
    /// Wraps the endpoint in Arc<Mutex<>> for thread-safe sharing
    pub fn set_endpoint(&mut self, endpoint: Box<dyn Endpoint>) {
        self.endpoint = Some(Arc::new(Mutex::new(endpoint)));
    }
    
    /// Add a child node with the given name
    pub fn add_child(&mut self, name: &str, node: Node) {
        self.children.insert(name.to_string(), node);
    }
    
    /// Get names of all child nodes
    pub fn child_names(&self) -> Vec<String> {
        self.children.keys().cloned().collect()
    }
    
    /// List child nodes at a given path - traverses directly without find_handler
    ///
    /// Works even without endpoint at the path (like Linux directories).
    ///
    /// # Arguments
    /// * `path` - The path to list children at (e.g., "/" or "/shell")
    ///
    /// # Returns
    /// List of child node names, or empty list if path not found
    pub fn list_nodes_at(&self, path: &str) -> Vec<String> {
        let segments: Vec<String> = path
            .split('/')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();
        let mut current = self;
        for seg in &segments {
            if let Some(child) = current.children.get(seg) {
                current = child;
            } else {
                return vec![];
            }
        }
        current.child_names()
    }
    
    /// List endpoints at a given path - traverses directly without find_handler
    ///
    /// Works even without endpoint at the path (like Linux directories).
    pub fn list_endpoints_at(&self, path: &str) -> Vec<EndpointInfo> {
        let segments: Vec<String> = path
            .split('/')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();
        let mut current = self;
        for seg in &segments {
            if let Some(child) = current.children.get(seg) {
                current = child;
            } else {
                return vec![];
            }
        }
        current.endpoint_names()
    }
    
    /// Get all endpoints at this node and in children
    pub fn endpoint_names(&self) -> Vec<EndpointInfo> {
        let mut endpoints = Vec::new();
        
        if let Some(ref e) = self.endpoint {
            if let Ok(ep) = e.lock() {
                endpoints.push(EndpointInfo {
                    name: ep.name().to_string(),
                    path: self.path.clone(),
                    endpoint_type: ep.endpoint_type(),
                });
            }
        }
        
        for (name, child) in &self.children {
            let mut child_endpoints = child.endpoint_names();
            for ep in &mut child_endpoints {
                ep.path = format!("{}/{}", self.path, name);
                endpoints.push(ep.clone());
            }
        }
        
        endpoints
    }
    
    /// Get all leaf paths (nodes with endpoint but no children)
    pub fn leaf_paths(&self) -> Vec<String> {
        let mut paths = Vec::new();

        if self.endpoint.is_some() && self.children.is_empty() {
            paths.push(self.path.clone());
        }

        for (name, child) in &self.children {
            let mut child_leaves = child.leaf_paths();
            for path in &mut child_leaves {
                *path = if self.path == "/" {
                    format!("/{}", name)
                } else {
                    format!("{}/{}", self.path, name)
                };
                paths.push(path.clone());
            }
        }

        paths
    }
    
    /// Get info about this node
    pub fn node_info(&self) -> NodeInfo {
        NodeInfo {
            path: self.path.clone(),
            is_leaf: self.endpoint.is_some() && self.children.is_empty(),
            has_children: !self.children.is_empty(),
            endpoints: self.endpoint_names().iter().map(|e| e.name.clone()).collect(),
        }
    }
}

/// Tree structure for routing - contains the root node.
#[allow(dead_code)]
pub struct Tree {
    pub root: Node,
}

impl fmt::Debug for Tree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tree")
            .field("root", &self.root.path)
            .finish()
    }
}

impl Tree {
    /// Create a new empty tree
    pub fn new() -> Self {
        Self { root: Node::new("/") }
    }
    
    /// Add an endpoint at the given path
    /// 
    /// # Arguments
    /// * `path` - The path where to register the endpoint (e.g., "/shell", "/tty")
    /// * `endpoint` - The endpoint to register
    pub fn add_endpoint(&mut self, path: &str, endpoint: Box<dyn Endpoint>) {
        let segments = path_segments(path);
        
        if segments.is_empty() {
            self.root.set_endpoint(endpoint);
            return;
        }
        
        let mut current = &mut self.root;
        let mut endpoint_opt: Option<Box<dyn Endpoint>> = Some(endpoint);
        
        for (i, segment) in segments.iter().enumerate() {
            let is_last = i == segments.len() - 1;
            
            if !current.children.contains_key(segment) {
                let parent_path = if i == 0 {
                    String::from("/")
                } else {
                    segments[..i].join("/")
                };
                let new_path = if parent_path == "/" {
                    format!("/{}", segment)
                } else {
                    format!("{}/{}", parent_path, segment)
                };
                current.add_child(segment, Node::new(&new_path));
            }
            
            current = current.children.get_mut(segment).unwrap();
            
            if is_last {
                if let Some(ep) = endpoint_opt.take() {
                    current.set_endpoint(ep);
                }
            }
        }
    }
    
    /// Find the handler for a given path using longest-prefix matching
    /// 
    /// Returns the endpoint and the matched path
    pub fn find_handler(&self, path: &str) -> Option<(Arc<Mutex<Box<dyn Endpoint>>>, &str)> {
        if path == "/" {
            return self.root.endpoint.as_ref().map(|e| (e.clone(), "/"));
        }
        
        let segments = path_segments(path);
        let mut current = &self.root;
        let mut remaining = segments.as_slice();
        let mut handler_path = "";
        
        while !remaining.is_empty() {
            if let Some(child) = current.children.get(&remaining[0].to_string()) {
                current = child;
                remaining = &remaining[1..];
                handler_path = &current.path;
            } else {
                break;
            }
        }
        
        current.endpoint.as_ref().map(|e| (e.clone(), handler_path))
    }
    
    /// List child nodes at a given path using direct tree traversal.
    ///
    /// Unlike `find_handler()`, this works even without an endpoint at the path,
    /// making "/" and other directories traversable like Linux directories.
    ///
    /// # Example
    /// ```
    /// let tree = Tree::new();
    /// tree.add_endpoint("/shell", Box::new(RemoteShell::new("shell")));
    /// let names = tree.list_nodes("/").unwrap();  // ["shell"]
    /// ```
    pub fn list_nodes(&self, path: &str) -> Result<Vec<String>, String> {
        // Use direct traversal - works without endpoint
        let names = self.root.list_nodes_at(path);
        Ok(names)
    }
    
    /// List all endpoints at a given path.
    ///
    /// Works even without endpoint at the path.
    pub fn list_endpoints(&self, path: &str) -> Result<Vec<EndpointInfo>, String> {
        let endpoints = self.root.list_endpoints_at(path);
        Ok(endpoints)
    }
    
    /// List all leaf paths in the tree.
    pub fn list_leaves(&self) -> Vec<String> {
        self.root.leaf_paths()
    }

    /// List child nodes at a given path - traverses directly without find_handler
    pub fn list_nodes_at(&self, path: &str) -> Vec<String> {
        let segments: Vec<String> = path
            .split('/')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();
        let mut current = &self.root;
        for seg in &segments {
            if let Some(child) = current.children.get(seg) {
                current = child;
            } else {
                return vec![];
            }
        }
        current.child_names()
    }

    /// List endpoints at a given path - traverses directly without find_handler
    pub fn list_endpoints_at(&self, path: &str) -> Vec<EndpointInfo> {
        let segments: Vec<String> = path
            .split('/')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();
        let mut current = &self.root;
        for seg in &segments {
            if let Some(child) = current.children.get(seg) {
                current = child;
            } else {
                return vec![];
            }
        }
        current.endpoint_names()
    }
    
    /// Get information about a node at the given path
    pub fn get_info(&self, path: &str) -> Result<NodeInfo, String> {
        let segments = path_segments(path);
        let mut current = &self.root;
        
        for segment in &segments {
            if let Some(child) = current.children.get(segment) {
                current = child;
            } else {
                return Err(format!("path not found: {}", path));
            }
        }
        
        Ok(current.node_info())
    }
    
    /// Open a stream to an endpoint at the given path
    /// 
    /// # Arguments
    /// * `path` - The path to open stream to
    /// * `src_path` - The source path for the stream
    /// 
    /// # Returns
    /// The stream ID on success
    pub fn open_stream(&mut self, path: &str, src_path: &str) -> Result<u16, String> {
        // First find the handler and matched path
        let (handler, matched_path) = self.find_handler(path)
            .ok_or_else(|| format!("path not found: {}", path))?;
        
        let segments = path_segments(matched_path);
        
        // Collect segment names first, then use indices to navigate
        // This avoids borrow issues by not holding references across operations
        let mut path_indices: Vec<String> = Vec::new();
        
        {
            let mut current = &self.root;
            for segment in &segments {
                if let Some(child) = current.children.get(segment) {
                    path_indices.push(segment.clone());
                    current = child;
                } else {
                    return Err(format!("node not found: {}", segment));
                }
            }
        }
        
        // Now navigate again with indices and get next_stream_id
        let stream_id = {
            let mut current = &mut self.root;
            for segment in &path_indices {
                current = current.children.get_mut(segment).unwrap();
            }
            let sid = current.next_stream_id;
            current.next_stream_id = current.next_stream_id.wrapping_add(1);
            sid
        };
        
        // Call handler's on_stream_open with locked mutex
        let stream_id = {
            let mut handler = handler.lock().map_err(|e| e.to_string())?;
            handler.on_stream_open(stream_id, src_path)
                .ok_or_else(|| "endpoint rejected stream".to_string())?
        };
        
        // Store stream info in the node
        {
            let mut current = &mut self.root;
            for segment in &path_indices {
                current = current.children.get_mut(segment).unwrap();
            }
            current.streams.insert(stream_id, Stream {
                stream_id,
                dst_path: path.to_string(),
                src_path: src_path.to_string(),
            });
        }
        
        Ok(stream_id)
    }
    
    #[allow(dead_code)]
    /// Find the index path to a node given segment names
    fn find_node_index(&self, segments: &[String]) -> Result<Vec<String>, String> {
        let mut current = &self.root;
        let mut path = Vec::new();
        
        for segment in segments {
            if let Some(child) = current.children.get(segment) {
                path.push(segment.clone());
                current = child;
            } else {
                return Err(format!("segment not found: {}", segment));
            }
        }
        
        Ok(path)
    }
    
    /// Get a mutable reference to a node at the given path
    #[allow(dead_code)]
    fn get_node_mut(&mut self, path: &[String]) -> Result<&mut Node, String> {
        let mut current = &mut self.root;
        
        for segment in path {
            if let Some(child) = current.children.get_mut(segment) {
                current = child;
            } else {
                return Err(format!("node not found: {}", segment));
            }
        }
        
        Ok(current)
    }
    
    /// Route stream data to the appropriate handler
    pub fn route_stream_data(&mut self, header: &FrameHeader, data: &[u8]) -> Result<(), String> {
        let stream_id = header.stream_id.ok_or("no stream_id")?;
        
        // Find the node containing this stream
        fn find_stream_handler(node: &mut Node, sid: u16) -> Option<Arc<Mutex<Box<dyn Endpoint>>>> {
            if node.streams.get(&sid).is_some() {
                return node.endpoint.clone();
            }
            
            for child in node.children.values_mut() {
                if let Some(h) = find_stream_handler(child, sid) {
                    return Some(h);
                }
            }
            
            None
        }
        
        if let Some(handler) = find_stream_handler(&mut self.root, stream_id) {
            if let Ok(mut h) = handler.lock() {
                h.on_stream_data(stream_id, data);
            }
        }
        
        Ok(())
    }
    
    /// Close a stream
    pub fn close_stream(&mut self, stream_id: u16) -> Result<(), String> {
        fn find_and_close(node: &mut Node, sid: u16) -> bool {
            if node.streams.remove(&sid).is_some() {
                if let Some(ref ep) = node.endpoint {
                    if let Ok(mut h) = ep.lock() {
                        h.on_stream_close(sid);
                    }
                    return true;
                }
            }
            
            for child in node.children.values_mut() {
                if find_and_close(child, sid) {
                    return true;
                }
            }
            
            false
        }
        
        find_and_close(&mut self.root, stream_id)
            .then_some(())
            .ok_or_else(|| format!("stream not found: {}", stream_id))
    }
}

/// Split a path into segments
/// 
/// # Example
/// ```
/// assert_eq!(path_segments("/foo/bar"), vec!["foo", "bar"]);
/// assert_eq!(path_segments("/"), vec![]);
/// ```
fn path_segments(path: &str) -> Vec<String> {
    path.split('/')
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}