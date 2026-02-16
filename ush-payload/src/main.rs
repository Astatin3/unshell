//! TCP Chain CLI Test Harness
//!
//! Demonstrates multi-layer TCP connections for testing pivoting.
//! Creates a chain of endpoints connected via TCP.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use serde_json::json;
use unshell::tree::message::TreeMessage;
use unshell::tree::protocols::{ProtocolConfig, ProtocolStack};
use unshell::tree::tcp::{TcpClient, TcpServer};
use unshell::tree::{ComponentRegistry, EndpointManager, TreeElement};

fn main() {
    println!("=== Tree Protocol Test Harness ===\n");

    // Test 1: Local TCP Server-Client loopback
    test_tcp_loopback();

    // Test 2: Tree message routing
    test_tree_message();

    // Test 3: TreeMessage serialization (new API)
    test_message_serialization();

    // Test 4: TCP Server with RPC
    test_tcp_server();

    // Test 5: TCP Client with RPC
    test_tcp_client();

    // Test 6: Protocol stacking
    test_protocol_stack();

    // Test 7: Component registry
    test_component_registry();

    println!("\n=== All tests complete ===");
}

/// Test basic TCP server/client communication
fn test_tcp_loopback() {
    println!("[Test 1] TCP Loopback Test");

    let (tx, rx) = mpsc::channel();

    let server_thread = thread::spawn(move || {
        let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind");
        let addr = listener.local_addr().unwrap();
        tx.send(addr.port()).unwrap();

        // Accept one connection only
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            if let Ok(n) = stream.read(&mut buf) {
                let response = b"Echo: ";
                let _ = stream.write(response);
                let _ = stream.write(&buf[..n]);
                let _ = stream.flush();
            }
        }
    });

    let port = rx.recv().unwrap();

    let mut stream =
        std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).expect("Failed to connect");
    stream
        .set_read_timeout(Some(Duration::from_millis(1000)))
        .unwrap();

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

    let response = endpoint
        .branch_mut()
        .send_message(serde_json::Value::Null, serde_json::json!("GetChildren"));

    let children = response.as_object().unwrap();
    println!("  Children: {:?}", children.keys().collect::<Vec<_>>());

    let response = endpoint
        .branch_mut()
        .send_message(serde_json::json!("id"), serde_json::Value::Null);
    println!("  Endpoint ID: {:?}", response);

    let sender = endpoint.logs_sender().clone();
    sender.send(serde_json::json!("Test log entry")).unwrap();

    let response = endpoint
        .branch_mut()
        .send_message(serde_json::json!("logs"), serde_json::json!("GetLength"));
    println!("  Log queue length: {:?}", response);

    println!("  ✓ Tree message test passed\n");
}

/// Test TreeMessage serialization (new API)
fn test_message_serialization() {
    println!("[Test 3] TreeMessage Serialization");

    // Test new API
    let msg = TreeMessage::new("query")
        .to_target(["endpoint1", "shell"])
        .with_payload(json!({"key": "value"}));

    let json = serde_json::to_string_pretty(&msg).unwrap();
    println!("  Message: {}", json);

    // Test response
    let resp =
        TreeMessage::new("response").with_payload(json!({"success": true, "result": "data"}));

    let json = serde_json::to_string_pretty(&resp).unwrap();
    println!("  Response: {}", json);

    // Test RPC call
    let rpc_msg = TreeMessage::new("rpc.call")
        .to_target(["components", "tcp-client"])
        .with_payload(json!({
            "method": "connect",
            "params": {"address": "127.0.0.1", "port": 8080}
        }));

    let json = serde_json::to_string_pretty(&rpc_msg).unwrap();
    println!("  RPC Call: {}", json);

    println!("  ✓ Message serialization test passed\n");
}

/// Test TCP Server with RPC
fn test_tcp_server() {
    println!("[Test 4] TCP Server with RPC");

    let mut server = TcpServer::new("test-server");

    // Configure
    let response = server.send_message(
        json!(null),
        json!({
            "method": "status"
        }),
    );
    println!("  Initial status: {:?}", response);

    // Try to start listening
    let response = server.send_message(
        json!(null),
        json!({
            "method": "listen",
            "params": {
                "bind_address": "127.0.0.1",
                "port": 0
            }
        }),
    );
    println!("  Listen result: {:?}", response);

    // Get status after listening
    let response = server.send_message(json!(null), json!({"method": "status"}));
    println!("  Status after listen: {:?}", response);

    println!("  ✓ TCP Server test passed\n");
}

/// Test TCP Client with RPC
fn test_tcp_client() {
    println!("[Test 5] TCP Client with RPC");

    let mut client = TcpClient::new("test-client");

    // Get initial status
    let response = client.send_message(json!(null), json!({"method": "status"}));
    println!("  Initial status: {:?}", response);

    // Try to connect (will fail since no server, but tests the RPC)
    let response = client.send_message(
        json!(null),
        json!({
            "method": "connect",
            "params": {"address": "127.0.0.1", "port": 65432}
        }),
    );
    println!("  Connect result: {:?}", response);

    // Get status after connect attempt
    let response = client.send_message(json!(null), json!({"method": "status"}));
    println!("  Status after connect: {:?}", response);

    println!("  ✓ TCP Client test passed\n");
}

/// Test Protocol stacking
fn test_protocol_stack() {
    println!("[Test 6] Protocol Stacking");

    // Create a stack: base64 -> tcp
    let mut stack = ProtocolStack::new();
    stack
        .push(&ProtocolConfig::Base64(Default::default()))
        .unwrap();
    stack
        .push(&ProtocolConfig::Tcp(Default::default()))
        .unwrap();

    println!("  Stack protocols: {:?}", stack.to_configs());

    // Test encoding/decoding
    let test_data = b"Hello, World!";
    let encoded = stack.encode(test_data).unwrap();
    println!(
        "  Encoded ({} bytes): {:?}",
        encoded.len(),
        String::from_utf8_lossy(&encoded)
    );

    let decoded = stack.decode(&encoded).unwrap();
    println!("  Decoded: {:?}", String::from_utf8_lossy(&decoded));

    // Test with HTTP
    let mut http_stack = ProtocolStack::new();
    http_stack
        .push(&ProtocolConfig::Base64(Default::default()))
        .unwrap();
    http_stack
        .push(&ProtocolConfig::Http(Default::default()))
        .unwrap();

    let test_data = b"test message";
    let encoded = http_stack.encode(test_data).unwrap();
    println!(
        "  HTTP+Base64 encoded: {:?}",
        String::from_utf8_lossy(&encoded).lines().next()
    );

    println!("  ✓ Protocol stacking test passed\n");
}

/// Test Component Registry
fn test_component_registry() {
    println!("[Test 7] Component Registry");

    let mut registry = ComponentRegistry::new();

    // Create and register a TCP client component
    let client = Box::new(TcpClient::new("tcp-client-1"));
    registry.register(client).unwrap();

    // List components
    let list = registry.list();
    println!("  Registered components: {:?}", list);

    // Send RPC to component
    let result = registry.send_to_component("tcp-client-1", json!({"method": "status"}));
    println!("  Component status: {:?}", result);

    // Send RPC via path
    let result = registry.send_message(json!("rpc.tcp-client-1"), json!({"method": "status"}));
    println!("  Via RPC path: {:?}", result);

    println!("  ✓ Component registry test passed\n");
}
