//! # TTY Leaf
//! 
//! This module provides PTY-based terminal sessions for the unshell protocol.
//! It supports opening pseudo-terminals and streaming data to/from them.

use crate::protocol::{TreeRequest, TreeResponse, EndpointType};
use crate::tree::Endpoint;
use std::boxed::Box;
use std::result::Result;
use std::collections::HashMap;

/// A PTY session - represents an active terminal session
#[allow(dead_code)]
pub struct PtySession { 
    /// Stream ID for this session
    pub stream_id: u16, 
    /// Master file descriptor for the PTY
    pub master: std::os::unix::io::RawFd, 
    /// Child process PID
    pub child_pid: u32 
}

/// TTY endpoint - provides PTY streaming functionality
pub struct TTY {
    name: String,
    sessions: HashMap<u16, Box<PtySession>>,
    #[allow(dead_code)]
    next_id: u16,
}

impl TTY {
    /// Create a new TTY endpoint
    pub fn new(name: &str) -> Self {
        Self { 
            name: name.to_string(), 
            sessions: HashMap::new(), 
            next_id: 1 
        }
    }
    
    /// Open a new PTY session
    /// 
    /// # Arguments
    /// * `stream_id` - The stream ID for this session
    /// 
    /// # Returns
    /// Ok(()) on success, Err(message) on failure
    fn open_pty(&mut self, stream_id: u16) -> Result<(), String> {
        // Open PTY master - must be unsafe
        let master = unsafe { libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY) };
        if master < 0 { 
            return Err("failed to open PTY".to_string()); 
        }
        
        // Grant PTY access - unsafe
        if unsafe { libc::grantpt(master) } != 0 { 
            unsafe { libc::close(master); }
            return Err("failed to grant PTY".to_string()); 
        }
        
        // Unlock PTY - unsafe
        if unsafe { libc::unlockpt(master) } != 0 { 
            unsafe { libc::close(master); }
            return Err("failed to unlock PTY".to_string()); 
        }
        
        // Get slave name - unsafe but returns pointer we need to check
        let slave_name = unsafe {
            let ptr = libc::ptsname(master);
            if ptr.is_null() { 
                libc::close(master); 
                return Err("failed to get PTY name".to_string()); 
            }
            std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned()
        };
        
        // Fork - unsafe
        let pid = unsafe { libc::fork() };
        if pid < 0 { 
            unsafe { libc::close(master); }
            return Err("fork failed".to_string()); 
        }
        
        if pid == 0 {
            // Child process - set up slave PTY and exec shell
            unsafe { libc::close(master); }
            
            let slave = unsafe { libc::open(slave_name.as_ptr() as *const libc::c_char, libc::O_RDWR) };
            if slave < 0 { 
                unsafe { libc::exit(1); }
            }
            
            // Set controlling terminal - unsafe
            unsafe { libc::ioctl(slave, libc::TIOCSCTTY, 0); }
            
            // Redirect stdio - unsafe
            unsafe { 
                libc::dup2(slave, libc::STDIN_FILENO); 
                libc::dup2(slave, libc::STDOUT_FILENO); 
                libc::dup2(slave, libc::STDERR_FILENO); 
                libc::close(slave); 
            }
            
            // Exec shell - unsafe
            unsafe { 
                libc::execl(
                    "/bin/sh\0".as_ptr() as *const libc::c_char, 
                    "sh\0".as_ptr() as *const libc::c_char, 
                    std::ptr::null::<libc::c_char>()
                );
            }
            
            // If exec fails, exit
            unsafe { libc::exit(1); }
        }
        
        // Parent - store session
        self.sessions.insert(stream_id, Box::new(PtySession { 
            stream_id, 
            master, 
            child_pid: pid as u32 
        }));
        Ok(())
    }
    
    /// Write data to a PTY session
    /// 
    /// # Arguments
    /// * `stream_id` - The stream ID
    /// * `data` - The data to write
    /// 
    /// # Returns
    /// Ok(()) on success, Err(message) on failure
    fn write_to_pty(&mut self, stream_id: u16, data: &[u8]) -> Result<(), String> {
        let session = self.sessions.get_mut(&stream_id).ok_or("session not found")?;
        let written = unsafe { 
            libc::write(
                session.master, 
                data.as_ptr() as *const libc::c_void, 
                data.len()
            ) 
        };
        if written < 0 { 
            return Err("write failed".to_string()); 
        }
        Ok(())
    }
    
    /// Close a PTY session
    /// 
    /// # Arguments
    /// * `stream_id` - The stream ID to close
    fn close_pty(&mut self, stream_id: u16) {
        if let Some(session) = self.sessions.remove(&stream_id) {
            // Send SIGTERM to child - unsafe
            unsafe { libc::kill(session.child_pid as i32, libc::SIGTERM); }
            
            // Wait for child - unsafe
            let mut status: libc::c_int = 0;
            unsafe { libc::waitpid(session.child_pid as i32, &mut status, 0); }
            
            // Close master - unsafe
            unsafe { libc::close(session.master); }
        }
    }
}

impl Endpoint for TTY {
    /// Handle a request - TTY only supports exec for basic commands
    fn handle_request(&mut self, request: &TreeRequest, _src_path: &str) -> Result<TreeResponse, String> {
        match request {
            TreeRequest::Exec { cmd } => {
                use std::process::{Command, Stdio};
                let output = Command::new("sh")
                    .args(["-c", cmd])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
                    .map_err(|e| e.to_string())?;
                Ok(TreeResponse::ExecOutput { 
                    exit_code: output.status.code().unwrap_or(-1), 
                    stdout: output.stdout, 
                    stderr: output.stderr 
                })
            }
            _ => Err("use stream for TTY".to_string()),
        }
    }
    
    /// Handle stream open - creates a new PTY session
    fn on_stream_open(&mut self, stream_id: u16, _src_path: &str) -> Option<u16> {
        self.open_pty(stream_id).ok().map(|_| stream_id)
    }
    
    /// Handle stream data - writes to PTY
    fn on_stream_data(&mut self, stream_id: u16, data: &[u8]) -> bool {
        self.write_to_pty(stream_id, data).ok();
        true
    }
    
    /// Handle stream close - closes PTY session
    fn on_stream_close(&mut self, stream_id: u16) { 
        self.close_pty(stream_id); 
    }
    
    /// Get endpoint type
    fn endpoint_type(&self) -> EndpointType { 
        EndpointType::Stream 
    }
    
    /// Get endpoint name
    fn name(&self) -> &str { 
        &self.name 
    }
}