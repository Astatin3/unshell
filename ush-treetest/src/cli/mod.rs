//! # CLI Module
//! 
//! This module provides the interactive CLI for the unshell tree protocol testbed.
//! It supports both local tree operations and remote connections.

use crate::protocol::{
    FrameType, TreeRequest, TreeResponse, TcpTransport, Transport,
    make_request, make_stream_open, make_stream_data, make_stream_close,
    make_handshake,
};
use crate::tree::Tree;
use crate::leaves::{RemoteShell, TTY};
use std::string::String;
use std::vec::Vec;
use std::boxed::Box;
use std::result::Result;

/// CLI state - manages connection and local tree
pub struct Cli {
    transport: Option<TcpTransport>,
    tree: Tree,
    current_path: String,
    request_id: u64,
    #[allow(dead_code)]
    stream_id: u16,
    streams: Vec<StreamState>,
    base_path: String,
    mode: CliMode,
}

/// CLI operation mode
#[derive(Debug, Clone, Copy)]
enum CliMode { Local, Connected }

/// State of an open stream
#[derive(Debug)]
#[allow(dead_code)]
struct StreamState { stream_id: u16, path: String }

impl Cli {
    /// Create a new CLI with a local tree
    pub fn new() -> Self {
        let mut tree = Tree::new();
        tree.add_endpoint("/shell", Box::new(RemoteShell::new("shell")));
        tree.add_endpoint("/tty", Box::new(TTY::new("tty")));
        Self { 
            transport: None, 
            tree, 
            current_path: String::from("/"), 
            request_id: 1, 
            stream_id: 1, 
            streams: Vec::new(), 
            base_path: String::from("/"), 
            mode: CliMode::Local 
        }
    }
    
    /// Get next request ID
    fn next_request_id(&mut self) -> u64 { 
        let id = self.request_id; 
        self.request_id += 1; 
        id 
    }
    
    /// Get next stream ID
    #[allow(dead_code)]
    fn next_stream_id(&mut self) -> u16 { 
        let id = self.stream_id; 
        self.stream_id = self.stream_id.wrapping_add(1); 
        id 
    }
    
    /// List nodes at a path
    pub fn list_nodes(&self, path: Option<&str>) -> Result<Vec<String>, String> {
        let path = path.unwrap_or(&self.current_path);
        self.tree.list_nodes(path)
    }
    
    /// List endpoints at a path
    pub fn list_endpoints(&self, path: Option<&str>) -> Result<Vec<crate::protocol::EndpointInfo>, String> {
        let path = path.unwrap_or(&self.current_path);
        self.tree.list_endpoints(path)
    }
    
    /// List all leaf paths
    pub fn list_leaves(&self) -> Vec<String> { 
        self.tree.list_leaves() 
    }
    
    /// Get info about a node
    pub fn get_info(&self, path: &str) -> Result<crate::protocol::NodeInfo, String> { 
        self.tree.get_info(path) 
    }
    
    /// Execute a command locally on the tree
    pub fn exec_local(&mut self, path: &str, cmd: &str) -> Result<TreeResponse, String> {
        let (handler, matched_path) = self.tree.find_handler(path)
            .ok_or_else(|| format!("path not found: {}", path))?;
        
        let request = TreeRequest::Exec { cmd: cmd.to_string() };
        
        // Lock the handler and make the request
        let mut handler = handler.lock().map_err(|e| e.to_string())?;
        handler.handle_request(&request, matched_path)
    }
    
    /// Connect to a remote server
    pub fn connect(&mut self, addr: &str) -> Result<(), String> {
        let transport = TcpTransport::connect(addr).map_err(|e| e.to_string())?;
        self.transport = Some(transport);
        self.mode = CliMode::Connected;
        self.do_handshake()
    }
    
    /// Perform handshake with remote server
    fn do_handshake(&mut self) -> Result<(), String> {
        let transport = self.transport.as_mut().ok_or("not connected")?;
        let (header, payload) = make_handshake(vec![self.current_path.clone()]);
        transport.send_frame(&header, Some(&payload)).map_err(|e| e.to_string())?;
        let (header, payload) = transport.recv_frame().map_err(|e| e.to_string())?;
        if header.frame_type != FrameType::HandshakeAck { return Err("unexpected response type".to_string()); }
        let ack = crate::protocol::HandshakeAck::from_bytes(&payload).map_err(|e| e.to_string())?;
        if !ack.accepted { return Err("handshake rejected".to_string()); }
        self.base_path = ack.assigned_base_path.clone();
        Ok(())
    }
    
    /// Send a request to the remote server
    pub fn send_request(&mut self, dst_path: &str, request: &TreeRequest) -> Result<TreeResponse, String> {
        // Get request_id first to avoid borrow issues
        let request_id = self.next_request_id();
        
        let transport = self.transport.as_mut().ok_or("not connected")?;
        
        let full_path = if dst_path.starts_with('/') { 
            dst_path.to_string() 
        } else { 
            format!("{}/{}", self.current_path, dst_path) 
        };
        
        let (header, payload) = make_request(&full_path, &self.base_path, request_id, request);
        transport.send_frame(&header, Some(&payload)).map_err(|e| e.to_string())?;
        
        let (header, payload) = transport.recv_frame().map_err(|e| e.to_string())?;
        if header.frame_type != FrameType::Response { return Err("unexpected response type".to_string()); }
        
        let response = TreeResponse::from_bytes(&payload).map_err(|e| e.to_string())?;
        Ok(response)
    }
    
    /// Open a stream to a remote path
    pub fn open_stream(&mut self, dst_path: &str) -> Result<u16, String> {
        // Get request_id first
        let request_id = self.next_request_id();
        
        let transport = self.transport.as_mut().ok_or("not connected")?;
        
        let full_path = if dst_path.starts_with('/') { 
            dst_path.to_string() 
        } else { 
            format!("{}/{}", self.current_path, dst_path) 
        };
        
        let header = make_stream_open(&full_path, &self.base_path, request_id);
        transport.send_frame(&header, None).map_err(|e| e.to_string())?;
        
        let (header, payload) = transport.recv_frame().map_err(|e| e.to_string())?;
        if header.frame_type != FrameType::Response { return Err("unexpected response type".to_string()); }
        
        let response = TreeResponse::from_bytes(&payload).map_err(|e| e.to_string())?;
        
        match response { 
            TreeResponse::StreamOpened { stream_id } => { 
                self.streams.push(StreamState { stream_id, path: full_path }); 
                Ok(stream_id) 
            } 
            _ => Err("expected StreamOpened".to_string()) 
        }
    }
    
    /// Send data on a stream
    pub fn send_stream_data(&mut self, stream_id: u16, data: &[u8]) -> Result<(), String> {
        let transport = self.transport.as_mut().ok_or("not connected")?;
        let (header, payload) = make_stream_data(stream_id, data);
        transport.send_frame(&header, Some(&payload)).map_err(|e| e.to_string())
    }
    
    /// Close a stream
    pub fn close_stream(&mut self, stream_id: u16) -> Result<(), String> {
        let transport = self.transport.as_mut().ok_or("not connected")?;
        let header = make_stream_close(stream_id);
        transport.send_frame(&header, None).map_err(|e| e.to_string())?;
        self.streams.retain(|s| s.stream_id != stream_id);
        Ok(())
    }
    
    /// Check if connected to remote
    pub fn is_connected(&self) -> bool { 
        matches!(self.mode, CliMode::Connected) 
    }
    
    /// Get current path
    pub fn current_path(&self) -> &str { 
        &self.current_path 
    }
    
    /// Set current path
    pub fn set_path(&mut self, path: &str) { 
        self.current_path = path.to_string(); 
    }
}

/// Parse and execute a CLI command
/// 
/// # Arguments
/// * `cli` - The CLI state
/// * `line` - The command line to parse
/// 
/// # Returns
/// Ok(output) on success, Err(error) on failure
pub fn parse_and_execute(cli: &mut Cli, line: &str) -> Result<String, String> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.is_empty() { return Ok(String::new()); }
    
    match parts[0] {
        "ls" | "list" => { 
            let path = parts.get(1).map(|s| *s); 
            let names = cli.list_nodes(path)?; 
            Ok(names.join("\n")) 
        }
        "endpoints" => { 
            let path = parts.get(1).map(|s| *s); 
            let eps = cli.list_endpoints(path)?; 
            let mut output = String::new(); 
            for ep in &eps { 
                output.push_str(&format!("{} ({:?}) at {}\n", ep.name, ep.endpoint_type, ep.path)); 
            } 
            Ok(output) 
        }
        "leaves" => { 
            Ok(cli.list_leaves().join("\n")) 
        }
        "info" => { 
            if parts.len() < 2 { return Err("usage: info <path>".to_string()); } 
            let info = cli.get_info(parts[1])?; 
            Ok(format!("{:?}", info)) 
        }
        "exec" => {
            if parts.len() < 3 { return Err("usage: exec <path> <command>".to_string()); }
            let path = parts[1]; 
            let cmd = parts[2..].join(" ");
            if cli.is_connected() { 
                let request = TreeRequest::Exec { cmd: cmd.clone() }; 
                let response = cli.send_request(path, &request)?; 
                format_response(response) 
            } else { 
                let response = cli.exec_local(path, &cmd)?; 
                format_response(response) 
            }
        }
        "cd" => { 
            if parts.len() < 2 { return Err("usage: cd <path>".to_string()); } 
            let path = parts[1]; 
            if cli.get_info(path).is_ok() { 
                cli.set_path(path); 
                Ok(format!("changed to {}", path)) 
            } else { 
                Err(format!("path not found: {}", path)) 
            } 
        }
        "pwd" => { 
            Ok(cli.current_path().to_string()) 
        }
        "connect" => { 
            if parts.len() < 2 { return Err("usage: connect <host:port>".to_string()); } 
            cli.connect(parts[1])?; 
            Ok(format!("connected to {}", parts[1])) 
        }
        "stream" => { 
            if parts.len() < 2 { return Err("usage: stream <path>".to_string()); } 
            if !cli.is_connected() { return Err("not connected".to_string()); } 
            let stream_id = cli.open_stream(parts[1])?; 
            Ok(format!("opened stream {} to {}", stream_id, parts[1])) 
        }
        "close" => { 
            if parts.len() < 2 { return Err("usage: close <stream_id>".to_string()); } 
            let stream_id: u16 = parts[1].parse().map_err(|_| "invalid stream id".to_string())?; 
            cli.close_stream(stream_id)?; 
            Ok(format!("closed stream {}", stream_id)) 
        }
        "send" => { 
            if parts.len() < 3 { return Err("usage: send <stream_id> <data>".to_string()); } 
            let stream_id: u16 = parts[1].parse().map_err(|_| "invalid stream id".to_string())?; 
            let data = parts[2..].join(" "); 
            cli.send_stream_data(stream_id, data.as_bytes())?; 
            Ok("sent".to_string()) 
        }
        "help" => { 
            Ok(HELP_TEXT.to_string()) 
        }
        _ => Err(format!("unknown command: {}", parts[0])),
    }
}

/// Format a TreeResponse for display
fn format_response(response: TreeResponse) -> Result<String, String> {
    match response {
        TreeResponse::NodeList { names } => Ok(names.join("\n")),
        TreeResponse::EndpointList { endpoints } => { 
            let mut output = String::new(); 
            for ep in endpoints { 
                output.push_str(&format!("{} ({:?})\n", ep.name, ep.endpoint_type)); 
            } 
            Ok(output) 
        }
        TreeResponse::LeafList { leaves } => Ok(leaves.join("\n")),
        TreeResponse::NodeInfo { info } => Ok(format!("path: {}\nis_leaf: {}\nhas_children: {}\nendpoints: {:?}", info.path, info.is_leaf, info.has_children, info.endpoints)),
        TreeResponse::ExecOutput { exit_code, stdout, stderr } => { 
            let mut output = String::new(); 
            output.push_str(&format!("exit code: {}\n", exit_code)); 
            if !stdout.is_empty() { output.push_str(&format!("stdout: {}\n", String::from_utf8_lossy(&stdout))); } 
            if !stderr.is_empty() { output.push_str(&format!("stderr: {}\n", String::from_utf8_lossy(&stderr))); } 
            Ok(output) 
        }
        TreeResponse::StreamOpened { stream_id } => Ok(format!("stream opened: {}", stream_id)),
    }
}

/// Help text for CLI commands
const HELP_TEXT: &str = r#"Commands:
  ls [path]           List child nodes
  endpoints [path]    List endpoints at path
  leaves              List all leaf paths
  info <path>         Get node info
  exec <path> <cmd>   Execute command at path
  cd <path>           Change current path
  pwd                 Print working path
  connect <host>      Connect to remote server
  stream <path>       Open stream to path
  send <id> <data>    Send data on stream
  close <id>          Close stream
  help                Show this help
"#;