//! # RemoteShell Leaf

use crate::protocol::{TreeRequest, TreeResponse, EndpointType};
use crate::tree::Endpoint;
use std::string::String;
use std::vec::Vec;
use std::result::Result;

pub struct RemoteShell { name: String }

impl RemoteShell {
    pub fn new(name: &str) -> Self { Self { name: name.to_string() } }
    fn execute(&self, cmd: &str) -> (i32, Vec<u8>, Vec<u8>) {
        use std::process::{Command, Stdio};
        match Command::new("sh").args(["-c", cmd]).stdout(Stdio::piped()).stderr(Stdio::piped()).output() {
            Ok(out) => (out.status.code().unwrap_or(-1), out.stdout, out.stderr),
            Err(e) => (-1, Vec::new(), format!("{}\n", e).into_bytes()),
        }
    }
}

impl Endpoint for RemoteShell {
    fn handle_request(&mut self, request: &TreeRequest, _src_path: &str) -> Result<TreeResponse, String> {
        match request {
            TreeRequest::Exec { cmd } => {
                let (exit_code, stdout, stderr) = self.execute(cmd);
                Ok(TreeResponse::ExecOutput { exit_code, stdout, stderr })
            }
            _ => Err("unsupported request".to_string()),
        }
    }
    fn on_stream_open(&mut self, _stream_id: u16, _src_path: &str) -> Option<u16> { None }
    fn on_stream_data(&mut self, _stream_id: u16, _data: &[u8]) -> bool { false }
    fn on_stream_close(&mut self, _stream_id: u16) {}
    fn endpoint_type(&self) -> EndpointType { EndpointType::Leaf }
    fn name(&self) -> &str { &self.name }
}