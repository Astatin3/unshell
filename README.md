![unshell logo](https://github.com/astatin3/unshell/blob/main/media/full_banner.png?raw=true)

UnShell is a tree-addressed RPC and data-exchange protocol designed for a hierarchy of endpoints. It provides a lightweight way to route calls, streams, and faults through a tree-structured network of nodes.

## Features

- **Tree-Based Routing**: Path-based addressing with longest-prefix matching.
- **RPC and Streaming**: Supports simple request-response (Call) and bidirectional streaming (Data).
- **Hierarchical Control**: Authority-restricted downward calls and upstream-only faults.
- **Introspection**: Mandatory discovery mechanism for endpoints and leaves.
- **no_std Compatible**: Core protocol implementation is designed for constrained environments.

## Architecture

The protocol is organized into:
- `unshell`: Core protocol logic, framing, and routing.
- `ush-treetest`: A testbed implementation for validating the protocol.
- `ush-obfuscate`: Utilities for code obfuscation.
- `base62`: Encoding utilities.

## Getting Started

### Build
```bash
cargo build
```

### Run Tests
```bash
cargo test
```

## Protocol Specification
For detailed information on the wire format and behavioral invariants, please refer to `PROTOCOL.md`.
