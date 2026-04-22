//! # Unshell Tree Protocol Testbed
//! 
//! This is a testbed implementation of a tree-based routing protocol for unshell.
//! It supports serving and connecting to tree endpoints, with leaves for RemoteShell
//! (command execution) and TTY (PTY streaming).

mod cli;
mod leaves;
mod protocol;
mod tree;

use crate::protocol::{FrameHeader, FrameType, TreeRequest, TreeResponse, make_response, make_handshake_ack, Transport};
use crate::tree::Tree;
use crate::leaves::{RemoteShell, TTY};
use crate::protocol::TcpTransport;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ush-treetest")]
#[command(about = "Unshell tree protocol testbed")]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,
    
    #[arg(short, long)]
    addr: Option<String>,
}

#[derive(Subcommand)]
enum Command {
    Serve {
        #[arg(default_value = "0.0.0.0:8080")]
        addr: String,
    },
    Connect {
        #[arg(default_value = "localhost:8080")]
        addr: String,
    },
    Cli {},
    Run {
        command: String,
    },
}

fn main() {
    let _ = env_logger::try_init();
    
    let args = Args::parse();
    
    match args.command {
        Some(Command::Serve { addr }) => {
            run_server(&addr);
        }
        Some(Command::Connect { addr }) => {
            run_client(&addr);
        }
        Some(Command::Run { command }) => {
            run_single_command(&command);
        }
        None | Some(Command::Cli {}) => {
            run_interactive();
        }
    }
}

fn run_server(addr: &str) {
    log::info!("Starting server on {}", addr);
    
    let tree = Arc::new(Mutex::new(Tree::new()));
    {
        let mut tree = tree.lock().unwrap();
        tree.add_endpoint("/shell", Box::new(RemoteShell::new("shell")));
        tree.add_endpoint("/tty", Box::new(TTY::new("tty")));
    }
    
    let listener = TcpTransport::listen(addr).expect("failed to bind");
    log::info!("Listening on {}", addr);
    
    loop {
        match TcpTransport::accept(&listener) {
            Ok(transport) => {
                log::info!("New connection from {:?}", transport.peer_addr());
                let tree = Arc::clone(&tree);
                std::thread::spawn(move || {
                    handle_connection(transport, tree);
                });
            }
            Err(e) => {
                log::error!("accept error: {:?}", e);
            }
        }
    }
}

fn handle_connection(mut transport: TcpTransport, tree: Arc<Mutex<Tree>>) {
    let (header, _payload) = match transport.recv_frame() {
        Ok(h) => h,
        Err(e) => {
            log::error!("recv error: {:?}", e);
            return;
        }
    };
    
    if header.frame_type != FrameType::Handshake {
        log::error!("expected handshake");
        return;
    }
    
    log::info!("Client connected");
    
    let (ack_header, ack_payload) = make_handshake_ack(true, "/client");
    transport.send_frame(&ack_header, Some(&ack_payload)).expect("send failed");
    
    loop {
        match transport.recv_frame() {
            Ok((header, payload)) => {
                let response = handle_frame(&header, &payload, &tree);
                
                if let Some(response) = response {
                    let (resp_header, resp_payload) = match response {
                        Ok((h, p)) => (h, p),
                        Err(e) => {
                            log::error!("handle error: {:?}", e);
                            break;
                        }
                    };
                    transport.send_frame(&resp_header, Some(&resp_payload)).expect("send failed");
                }
                
                if header.frame_type == FrameType::StreamClose {
                    break;
                }
            }
            Err(e) => {
                log::error!("recv error: {:?}", e);
                break;
            }
        }
    }
    
    log::info!("Connection closed");
}

/// Handle a single frame and return an optional response
/// 
/// # Arguments
/// * `header` - The frame header
/// * `payload` - The frame payload bytes
/// * `tree` - Shared access to the tree
/// 
/// # Returns
/// Some(Ok((header, payload))) for a response to send, Some(Err(e)) for an error, None for no response
fn handle_frame(header: &FrameHeader, payload: &[u8], tree: &Arc<Mutex<Tree>>) -> Option<Result<(FrameHeader, Vec<u8>), String>> {
    match header.frame_type {
        FrameType::Request => {
            let request: TreeRequest = match TreeRequest::from_bytes(payload) {
                Ok(r) => r,
                Err(e) => return Some(Err(e.to_string())),
            };
            
            let dst_path = header.dst_path.as_deref().unwrap_or("/");
            
            // Acquire lock for the entire request handling
            let mut tree = match tree.lock() {
                Ok(t) => t,
                Err(e) => return Some(Err(format!("lock error: {}", e))),
            };
            
            let response = match request {
                TreeRequest::ListNodes {} => {
                    let names = tree.list_nodes(dst_path).unwrap_or_default();
                    TreeResponse::NodeList { names }
                }
                TreeRequest::ListEndpoints {} => {
                    let endpoints = tree.list_endpoints(dst_path).unwrap_or_default();
                    TreeResponse::EndpointList { endpoints }
                }
                TreeRequest::ListLeaves {} => {
                    let leaves = tree.list_leaves();
                    TreeResponse::LeafList { leaves }
                }
                TreeRequest::GetInfo { path } => {
                    match tree.get_info(&path) {
                        Ok(info) => TreeResponse::NodeInfo { info },
                        Err(e) => return Some(Err(e)),
                    }
                }
                TreeRequest::Exec { ref cmd } => {
                    let (handler, matched_path) = match tree.find_handler(dst_path) {
                        Some(h) => h,
                        None => return Some(Err(format!("path not found: {}", dst_path))),
                    };
                    // Lock the handler and make the request
                    let result = {
                        let mut handler = match handler.lock() {
                            Ok(h) => h,
                            Err(e) => return Some(Err(format!("lock error: {}", e))),
                        };
                        handler.handle_request(&TreeRequest::Exec { cmd: cmd.clone() }, matched_path)
                    };
                    match result {
                        Ok(resp) => resp,
                        Err(e) => return Some(Err(e)),
                    }
                }
                TreeRequest::StreamOpen { path } => {
                    match tree.open_stream(&path, &header.src_path) {
                        Ok(stream_id) => TreeResponse::StreamOpened { stream_id },
                        Err(e) => return Some(Err(e)),
                    }
                }
                TreeRequest::Resize { .. } => {
                    return Some(Err("unsupported request: Resize".to_string()));
                }
            };
            
            Some(Ok(make_response(&header.src_path, header.request_id.unwrap_or(0), &response)))
        }
        
        FrameType::StreamOpen => {
            let dst_path = header.dst_path.as_deref().unwrap_or("/");
            let mut tree = match tree.lock() {
                Ok(t) => t,
                Err(e) => return Some(Err(format!("lock error: {}", e))),
            };
            match tree.open_stream(dst_path, &header.src_path) {
                Ok(stream_id) => {
                    let response = TreeResponse::StreamOpened { stream_id };
                    Some(Ok(make_response(&header.src_path, header.request_id.unwrap_or(0), &response)))
                }
                Err(e) => Some(Err(e)),
            }
        }
        
        FrameType::StreamData => {
            let mut tree = match tree.lock() {
                Ok(t) => t,
                Err(e) => return Some(Err(format!("lock error: {}", e))),
            };
            tree.route_stream_data(header, payload).ok();
            None
        }
        
        FrameType::StreamClose => {
            let mut tree = match tree.lock() {
                Ok(t) => t,
                Err(e) => return Some(Err(format!("lock error: {}", e))),
            };
            if let Some(stream_id) = header.stream_id {
                tree.close_stream(stream_id).ok();
            }
            None
        }
        
        _ => Some(Err("unsupported frame type".to_string())),
    }
}

fn run_client(addr: &str) {
    let mut cli = cli::Cli::new();
    
    if let Err(e) = cli.connect(addr) {
        eprintln!("Failed to connect: {}", e);
        return;
    }
    
    println!("Connected to {}", addr);
    run_cli_loop(&mut cli);
}

fn run_interactive() {
    let mut cli = cli::Cli::new();
    
    println!("Unshell Tree Protocol Testbed");
    println!("Type 'help' for commands\n");
    println!("Local tree with endpoints:");
    for leaf in cli.list_leaves() {
        println!("  {}", leaf);
    }
    println!();
    
    run_cli_loop(&mut cli);
}

fn run_cli_loop(cli: &mut cli::Cli) {
    loop {
        print!("{}> ", cli.current_path());
        io::stdout().flush().ok();
        
        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_err() {
            break;
        }
        
        let line = line.trim();
        
        if line.is_empty() {
            continue;
        }
        
        if line == "quit" || line == "exit" {
            break;
        }
        
        match cli::parse_and_execute(cli, line) {
            Ok(output) => {
                if !output.is_empty() {
                    println!("{}", output);
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }
    }
}

fn run_single_command(command: &str) {
    let mut cli = cli::Cli::new();
    
    match cli::parse_and_execute(&mut cli, command) {
        Ok(output) => {
            if !output.is_empty() {
                println!("{}", output);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}