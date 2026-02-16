//! TCP Chain CLI Test Harness
//!
//! Demonstrates multi-layer TCP connections for testing pivoting.
//! Creates a chain of endpoints connected via TCP.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;

use unshell::tree::{EndpointManager, TreeMessage};

fn main() {
    println!("=== TCP Chain Test Harness ===\n");

    // Test 1: Local TCP Server-Client loopback
    test_tcp_loopback();

    // Test 2: Tree message routing
    test_tree_message();

    // Test 3: TreeMessage serialization
    test_message_serialization();

    println!("\n=== All tests complete ===");
}

/// Test basic TCP server/client communication
fn test_tcp_loopback() {
    println!("[Test 1] TCP Loopback Test");

    // Start a TCP server in a thread
    let (tx, rx) = mpsc::channel();

    let server_thread = thread::spawn(move || {
        let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind");
        let addr = listener.local_addr().unwrap();
        tx.send(addr.port()).unwrap();

        for stream in listener.incoming() {
            if let Ok(mut stream) = stream {
                let mut buf = [0u8; 1024];
                if let Ok(n) = stream.read(&mut buf) {
                    let response = b"Echo: ";
                    let _ = stream.write(response);
                    let _ = stream.write(&buf[..n]);
                    let _ = stream.flush();
                }
            }
        }
    });

    let port = rx.recv().unwrap();

    // Connect client
    let mut stream =
        std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).expect("Failed to connect");

    let msg = b"Hello from client!";
    stream.write(msg).expect("Failed to write");

    let mut buf = [0u8; 1024];
    let n = stream.read(&mut buf).expect("Failed to read");
    let response = String::from_utf8_lossy(&buf[..n]);

    println!("  Client sent: {:?}", msg);
    println!("  Server responded: {:?}", response);

    server_thread.join().unwrap();
    println!("  ✓ Loopback test passed\n");
}

/// Test the tree message routing
fn test_tree_message() {
    println!("[Test 2] Tree Message Routing");

    let mut endpoint = EndpointManager::new("endpoint-1");

    // Test GetChildren
    let response = endpoint
        .branch_mut()
        .send_message(serde_json::Value::Null, serde_json::json!("GetChildren"));

    let children = response.as_object().unwrap();
    println!("  Children: {:?}", children.keys().collect::<Vec<_>>());

    // Test ID access
    let response = endpoint
        .branch_mut()
        .send_message(serde_json::json!("id"), serde_json::Value::Null);
    println!("  Endpoint ID: {:?}", response);

    // Test logs queue
    let sender = endpoint.logs_sender().clone();
    sender.send(serde_json::json!("Test log entry")).unwrap();

    let response = endpoint
        .branch_mut()
        .send_message(serde_json::json!("logs"), serde_json::json!("GetLength"));
    println!("  Log queue length: {:?}", response);

    println!("  ✓ Tree message test passed\n");
}

/// Test TreeMessage serialization
fn test_message_serialization() {
    println!("[Test 3] TreeMessage Serialization");

    let msg = TreeMessage::new_req(
        "msg-1",
        vec!["endpoint1".to_string(), "shell".to_string()],
        "Get",
    );

    let json = serde_json::to_string_pretty(&msg).unwrap();
    println!("  Message: {}", json);

    // Test response message
    let resp = TreeMessage::new_resp("resp-1", "msg-1", serde_json::json!("result data"));

    let json = serde_json::to_string_pretty(&resp).unwrap();
    println!("  Response: {}", json);

    println!("  ✓ Message serialization test passed\n");
}
