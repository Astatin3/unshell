//! # REPL Core
//!
//! The main interactive loop for the operator CLI.
//!
//! ## Flow
//!
//! ```text
//! run()
//!   ↓
//!   connect to router → handshake → register as operator node
//!   ↓
//!   start recv thread (router → operator messages)
//!   ↓
//!   main thread: readline loop
//!     parse command
//!     execute (may send TreeRequest over transport)
//!     print response
//! ```
//!
//! ## Threading model
//!
//! The transport is shared between:
//! - The main thread (sends requests, prints responses).
//! - A background recv thread (receives unsolicited messages from the router,
//!   e.g., node-connected notifications — future feature).
//!
//! In v1, the main thread does both send and receive synchronously (blocking
//! recv after each send). The recv thread is reserved for future async notifications.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

use unshell::protocol::{
    content, HandshakeAck, HandshakeMessage, NodeType,
    PacketHeader, PacketType, RequestType, TreeRequest,
};
use unshell::transport::tcp::TcpTransport;
use unshell::transport::Transport;

use crate::commands::{self, Command};
use crate::session::Session;

// ---------------------------------------------------------------------------
// Request ID counter
// ---------------------------------------------------------------------------

/// Monotonically increasing request ID generator.
///
/// Generates unique IDs so the operator can correlate responses to requests
/// in the future when multiple requests are in-flight concurrently.
static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(1);

fn next_request_id() -> u64 {
    REQUEST_COUNTER.fetch_add(1, Ordering::SeqCst)
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Start the operator REPL, connecting to `router_addr`.
///
/// Blocks until the user types `exit` or the connection is lost.
///
/// # Errors
///
/// Returns an error if the connection or handshake fails.
pub fn run(router_addr: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("UnShell operator console");
    println!("Connecting to {}...", router_addr);

    let mut transport = TcpTransport::connect(router_addr)?;
    let session_id = format!("sess{}", std::process::id());
    let base_path = format!("/operator/{session_id}");

    // Handshake
    let handshake = HandshakeMessage {
        node_id: session_id.clone(),
        node_type: NodeType::Operator,
        registered_paths: vec![base_path.clone()],
        platform: "operator".to_owned(),
    };
    let handshake_payload = rkyv::to_bytes::<rkyv::rancor::Error>(&handshake)
        .map_err(|e| format!("failed to serialise handshake: {e}"))?;
    let handshake_header = PacketHeader {
        dst_path: "/router".to_owned(),
        src_path: base_path.clone(),
        packet_type: PacketType::Handshake,
    };
    transport.send(&handshake_header, &handshake_payload)?;

    let (_, ack_payload) = transport.recv()?;
    let ack: HandshakeAck =
        rkyv::from_bytes::<HandshakeAck, rkyv::rancor::Error>(&ack_payload)
            .map_err(|e| format!("failed to deserialise ack: {e}"))?;

    if !ack.accepted {
        return Err(format!(
            "router rejected: {}",
            ack.rejection_reason.unwrap_or_default()
        )
        .into());
    }

    println!("Connected. Type 'help' for commands.");

    // Wrap transport in a Mutex for shared access
    let transport = Arc::new(Mutex::new(transport));

    // REPL state
    let mut current_session = Session::new("default", "/");
    let mut background_sessions: Vec<Session> = Vec::new();

    // Readline editor with history
    let mut rl = DefaultEditor::new()?;

    loop {
        let prompt = if current_session.current_path == "/" {
            "unshell> ".to_owned()
        } else {
            let short = current_session
                .current_path
                .trim_start_matches("/agents/")
                .trim_start_matches("/operator/");
            format!("unshell [{short}]> ")
        };

        let readline = rl.readline(&prompt);
        match readline {
            Ok(line) => {
                rl.add_history_entry(line.as_str())
                    .unwrap_or_default();

                match commands::parse(&line) {
                    Ok(None) => {} // empty / comment
                    Ok(Some(cmd)) => {
                        if !handle_command(
                            cmd,
                            &mut current_session,
                            &mut background_sessions,
                            &base_path,
                            &transport,
                        ) {
                            break; // exit command
                        }
                    }
                    Err(e) => println!("error: {e}"),
                }
            }
            Err(ReadlineError::Interrupted | ReadlineError::Eof) => {
                println!("Disconnecting...");
                break;
            }
            Err(e) => {
                eprintln!("readline error: {e}");
                break;
            }
        }
    }

    println!("Bye.");
    Ok(())
}

// ---------------------------------------------------------------------------
// Command handlers
// ---------------------------------------------------------------------------

/// Handle one parsed command.
///
/// Returns `false` if the REPL should exit, `true` to continue.
fn handle_command(
    cmd: Command,
    current_session: &mut Session,
    background_sessions: &mut Vec<Session>,
    base_path: &str,
    transport: &Arc<Mutex<TcpTransport>>,
) -> bool {
    match cmd {
        Command::Exit => return false,

        Command::Help => commands::print_help(),

        Command::Use(path) => {
            // Normalise: if no leading slash, prepend /agents/
            let resolved = if path.starts_with('/') {
                path
            } else {
                format!("/agents/{path}")
            };
            current_session.current_path = resolved;
            println!("current path: {}", current_session.current_path);
        }

        Command::List => {
            // Send GetProcedures to /router/nodes
            send_request_and_print(
                "/router/nodes",
                RequestType::GetProcedures,
                content::NONE,
                None,
                base_path,
                transport,
            );
        }

        Command::Ls(sub_path) => {
            let path = sub_path
                .as_deref()
                .map(|p| current_session.resolve(p))
                .unwrap_or_else(|| current_session.current_path.clone());
            send_request_and_print(
                &path,
                RequestType::GetProcedures,
                content::NONE,
                None,
                base_path,
                transport,
            );
        }

        Command::Read(sub_path) => {
            let path = current_session.resolve(&sub_path);
            send_request_and_print(
                &path,
                RequestType::Read,
                content::NONE,
                None,
                base_path,
                transport,
            );
        }

        Command::Call { path, data } => {
            let full_path = current_session.resolve(&path);
            send_request_and_print(
                &full_path,
                RequestType::CallProcedure,
                content::UTF8_STRING,
                data.as_deref(),
                base_path,
                transport,
            );
        }

        Command::Write { path, data } => {
            let full_path = current_session.resolve(&path);
            send_request_and_print(
                &full_path,
                RequestType::Write,
                content::UTF8_STRING,
                Some(&data),
                base_path,
                transport,
            );
        }

        Command::Background => {
            let mut session = current_session.clone();
            session.active = false;
            background_sessions.push(session);
            current_session.current_path = "/".to_owned();
            println!("session backgrounded. Type 'sessions' to list.");
        }

        Command::Sessions => {
            if background_sessions.is_empty() {
                println!("no background sessions");
            } else {
                for (i, sess) in background_sessions.iter().enumerate() {
                    println!("  [{i}] {} ({})", sess.name, sess.current_path);
                }
            }
        }
    }

    true
}

/// Send a `TreeRequest` and print the response.
fn send_request_and_print(
    dst_path: &str,
    request_type: RequestType,
    content_type: &str,
    data: Option<&str>,
    src_path: &str,
    transport: &Arc<Mutex<TcpTransport>>,
) {
    let request = TreeRequest {
        request_id: next_request_id(),
        request_type,
        content_type: content_type.to_owned(),
        data: data.map(|s| s.as_bytes().to_vec()).unwrap_or_default(),
    };

    let Ok(payload) = rkyv::to_bytes::<rkyv::rancor::Error>(&request) else {
        eprintln!("error: failed to serialise request");
        return;
    };

    let header = PacketHeader {
        dst_path: dst_path.to_owned(),
        src_path: src_path.to_owned(),
        packet_type: PacketType::Request,
    };

    let mut t = transport.lock().expect("transport lock poisoned");

    if let Err(e) = t.send(&header, &payload) {
        eprintln!("send error: {e}");
        return;
    }

    match t.recv() {
        Ok((_, resp_payload)) => {
            match rkyv::from_bytes::<unshell::protocol::TreeResponse, rkyv::rancor::Error>(
                &resp_payload,
            ) {
                Ok(resp) => {
                    if resp.data.is_empty() {
                        println!("[{:?}]", resp.status);
                    } else if let Ok(text) = std::str::from_utf8(&resp.data) {
                        println!("{text}");
                    } else {
                        println!("[{} bytes, content-type: {}]", resp.data.len(), resp.content_type);
                    }
                }
                Err(e) => eprintln!("error: failed to deserialise response: {e}"),
            }
        }
        Err(e) => eprintln!("recv error: {e}"),
    }
}
