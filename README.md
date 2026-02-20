# unshell

A fully modular, pluggable framework for building cross-platform endpoint agents that integrate with existing toolsets.

## Design Goals

- **100% Modular** - Every component is replaceable at runtime. Nothing is hardcoded.
- **Tool Integration** - Drop in Metasploit payloads, Cobalt Strike beacons, or any external implant
- **Cross-Platform** - Full Rust cross-compilation support for Windows, Linux, macOS, and embedded targets
- **Minimal Footprint** - Compile-time obfuscation and size optimization for stealthy payloads

## Philosophy

Nothing is fixed. Every part of the system is a plugin:

- **Transports** - TCP, HTTP, DNS, WebSocket, custom
- **Protocols** - Encryption, encoding, framing - all swappable
- **Payloads** - Metasploit, Cobalt Strike, custom - just load and run
- **Components** - Any Rust struct can be a module
- **Communication** - Tree-based routing with replaceable backends

## Architecture

```
unshell/
├── src/tree/           # Hierarchical message routing
│   ├── component.rs    # Component trait (implement for any module)
│   ├── endpoint.rs     # Endpoint manager
│   ├── protocols/      # Pluggable protocol stack
│   └── tcp/           # Example transport implementations
├── ush-obfuscate/     # Compile-time string obfuscation
└── ush-payload/       # Test harness
```

## Core Traits

Everything plugs into these abstractions:

### TreeElement - The Foundation

Every node in the tree implements this trait:

```rust
use serde_json::Value;
use unshell::tree::TreeElement;

pub trait TreeElement: Send + Sync {
    fn get_type(&self) -> Value;
    fn send_message(&mut self, target: Value, message: Value) -> Value;
}
```

- `get_type()` returns the element's type identifier
- `send_message()` handles incoming messages and returns responses

### Component - Extensible Modules

```rust
use unshell::tree::Component;
use serde_json::Value;

pub trait Component: Send + Sync {
    fn name(&self) -> &str;
    fn status(&self) -> Value;
    fn init(&mut self, config: Value) -> Result<(), String>;
    fn shutdown(&mut self) -> Result<(), String>;
}
```

**Important**: The `init()` method should only configure the component, not establish connections. 
For TcpClient, use `auto_connect: true` in config if you need auto-connection after initialization.

### Protocol - Any Encoding Layer

```rust
use unshell::protocols::Protocol;

pub trait Protocol: Send + Sync {
    fn name(&self) -> &'static str;
    fn encode(&self, data: &[u8]) -> Result<Vec<u8>, ProtocolError>;
    fn decode(&self, data: &[u8]) -> Result<Vec<u8>, ProtocolError>;
}
```

### Transport - Any Connection

```rust
// Transports connect to networks - TCP, HTTP, DNS, custom
// Implement send/recv and register with the transport registry
```

### Payload - Any External Implant

```rust
// External payloads (Metasploit, Cobalt Strike, etc.) load as components
// They expose the same interface as native components
```

## Tree Message Protocol

Messages follow a JSON-based format defined in `src/tree/message.rs`:

```rust
// Create a request
let msg = TreeMessage::new("rpc.call")
    .to_target(["components", "tcp-client"])
    .with_payload(json!({
        "method": "connect",
        "params": {"address": "127.0.0.1", "port": 443}
    }));

// Send via protocol stack
let encoded = protocol_stack.encode_message(&msg)?;
```

### Message Types

| Type | Description |
|------|-------------|
| `req` | Request - expecting a response |
| `resp` | Response - reply to a request |
| `event` | Unsolicited notification |
| `stream` | Stream data message |

## ComponentRegistry Usage

```rust
use unshell::tree::{ComponentRegistry, Component};

// Create registry
let mut registry = ComponentRegistry::new();

// Register components
let client = Box::new(TcpClient::new("my-client"));
registry.register(client).unwrap();

// List components
let names = registry.list();

// Send to specific component
let result = registry.send_to_component("my-client", json!({"method": "status"}));

// Broadcast to all
let results = registry.broadcast(json!({"method": "status"}));

// Shutdown all gracefully
let shutdown_results = registry.shutdown_all();

// Remove component
registry.remove("my-client");
```

## Logging

The framework includes a feature-gated logging system. Use the logging macros:

```rust
use unshell::{info, warn, error};

// Info messages
info!("Component '{}' registered", name);

// Warnings
warn!("Component '{}' not found", name);

// Errors
error!("Connection failed: {}", err);
```

Enable logging with the `log` feature:
```toml
unshell = { path = ".", features = ["log"] }
```

## Module System

```rust
use unshell::{ModuleRuntime, Manager};
use unshell::config::RuntimeConfig;

// Define a module
pub struct MyModule;

// Implement runtime lifecycle
impl ModuleRuntime for MyModule {
    fn init(&mut self, manager: Arc<Mutex<Manager>>) -> Result<()>;
    fn is_running(&self) -> bool;
    fn kill(self: Box<Self>);
}

// Export via FFI for dynamic loading
#[unsafe(no_mangle)]
pub fn get_components() -> Vec<NamedComponent> {
    vec![NamedComponent { name: "mymodule", ... }]
}
```

Load compiled `.so`/`.dll` modules at runtime using `libloading` or in-memory via `memfd_create`.

## Protocol Stacking

Layer protocols arbitrarily:

```rust
use ush_payload::protocols::{ProtocolStack, ProtocolConfig};

// Create stack: base64 -> http -> tcp
let mut stack = ProtocolStack::new();
stack.push(&ProtocolConfig::Base64(Default::default())).unwrap();
stack.push(&ProtocolConfig::Http(Default::default())).unwrap();
stack.push(&ProtocolConfig::Tcp(Default::default())).unwrap();

// Encode: app -> base64 -> http -> tcp -> network
let encoded = stack.encode(data)?;

// Decode: network -> tcp -> http -> base64 -> app
let decoded = stack.decode(&encoded)?;
```

Order determines encoding: app → base64 → http → tcp → network

## Integration Examples

### Load a Metasploit Payload

```rust
// Load precompiled Metasploit .so
let module = Module::new("meterpreter.so")?;
// Or load from raw bytes (in-memory execution)
let module = Module::new_bytes(&meterpreter_bytes)?;
```

### Use Cobalt Strike Beacon

```rust
// Beacon loads as a component with standard interface
let beacon = CobaltBeacon::new(config);
component_registry.register(Box::new(beacon)).unwrap();
// Communicate via tree messages - same as any other component
```

### Custom Transport

```rust
// Implement Protocol trait
pub struct DnsTransport { ... }
impl Protocol for DnsTransport {
    fn encode(&self, data: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        // Encode as DNS TXT records
    }
    fn decode(&self, data: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        // Decode DNS responses
    }
}
// Register and use
stack.push(&ProtocolConfig::Custom { name: "dns", config: ... });
```

### Create a Custom Component

```rust
use unshell::tree::{Component, TreeElement, Branch};
use serde_json::{json, Value};

pub struct MyComponent {
    name: String,
    config: MyConfig,
}

impl Component for MyComponent {
    fn name(&self) -> &str { &self.name }
    
    fn status(&self) -> Value {
        json!({"active": true, "name": self.name})
    }
    
    fn init(&mut self, config: Value) -> Result<(), String> {
        // Configure only - don't connect here
        self.config = serde_json::from_value(config)?;
        Ok(())
    }
    
    fn shutdown(&mut self) -> Result<(), String> {
        Ok(())
    }
}

// Register in component registry
let mut registry = ComponentRegistry::new();
registry.register(Box::new(MyComponent::new("my-component"))).unwrap();
```

## Cross-Compilation

```bash
# Windows x64
rustup target add x86_64-pc-windows-gnu
cargo build --target x86_64-pc-windows-gnu

# ARM64 Linux
rustup target add aarch64-unknown-linux-gnu
cargo build --target aarch64-unknown-linux-gnu

# macOS
rustup target add x86_64-apple-darwin
cargo build --target x86_64-apple-darwin
```

## Building

```bash
# Standard build (~500KB)
cargo build

# Size-optimized (~50KB)
cargo build --profile minimize

# With obfuscation
cargo build --features obfuscate
```

## Testing

```bash
# Run the test harness
cd ush-payload
cargo run

# Run library tests
cargo test -p ush-payload --lib
```

## Obfuscation

Compile-time string obfuscation to evade static analysis:

```rust
use ush_obfuscate::symbol;

const API_KEY: &str = symbol!("SuperSecretKey123");
const C2_URL: &str = symbol!("https://C2Server/endpoint");
```

## Roadmap

- [ ] Protocol registry for runtime registration
- [ ] Payload loader for common frameworks
- [ ] Transport abstraction layer
- [ ] Hot-swap components at runtime

## Dependencies

- `libloading` - Dynamic library loading
- `serde_json` - Serialization
- `crossbeam-channel` - Message passing
- `base64` - Encoding
- `thiserror` - Error handling

## License

MIT / Apache-2.0

## Implementation Notes

### Obfuscation Macros

The `ush-obfuscate` crate provides two macros for string obfuscation:

- **`sym!()`** - For static strings that are used at runtime. Uses AES encryption.
- **`xor!()`** - For simple XOR obfuscation.

When the `obfuscate` feature is enabled, strings are encrypted at compile time. When disabled, they remain as plain strings.

### Using Sym Constants

To avoid redundant `sym!()` calls, define constants in `src/tree/symbols.rs`:

```rust
use crate::obfuscate::sym;

pub const MY_CONSTANT: &str = sym!("MyString");
```

Then use the constant in your code:

```rust
use unshell::tree::symbols::*;

json!({ KEY_NAME: "value" })
```

### Module Organization

- **`src/tree/`** - Core tree system (routing, components, messages)
- **`ush-payload/src/`** - Protocol implementations, TCP client/server, connection management

Protocol-specific and transport-specific code belongs in `ush-payload`, while the tree system handles hierarchical message routing only.

### Building for Size

```bash
./build.sh  # Builds with minimize profile and strips debug sections
```

This produces a ~200KB binary (may vary with content).

### Component Design Guidelines

1. **init() should configure, not connect**: Only establish connections if explicitly requested via config
2. **Use symbols for string constants**: All user-facing strings should use `sym!()` for obfuscation
3. **Log important operations**: Use the logging macros for registration, connection, and errors
4. **Return structured responses**: Use JSON with `success`, `result`, and `error` fields
