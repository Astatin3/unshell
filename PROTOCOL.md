# Tree Protocol Specification

## Overview

The Tree Protocol is a lightweight, extensible message format designed for hierarchical message routing in a modular C2 (Command & Control) system. It provides RPC-like interactions, streaming support, event notifications, and peer-to-peer pivoting capabilities.

## Design Principles

1. **Loose typing** - Actions and payloads are flexible JSON values, not strict enums
2. **Namespacing** - Use dot notation for action categorization (e.g., `rpc.call`, `stream.data`)
3. **Extensibility** - Add new capabilities without modifying core structure
4. **Simplicity** - Minimal required fields, optional metadata for flexibility

## Message Structure

```json
{
    "id": "uuid-string",
    "source": "path/to/sender",
    "target": "path/to/recipient",
    "action": "action.name",
    "payload": {},
    "routing": {},
    "meta": {}
}
```

### Field Definitions

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | No | Unique message identifier for correlation |
| `source` | string/array | No | Origin path for responses |
| `target` | string/array | No | Destination path for routing |
| `action` | value | Yes | Operation to perform |
| `payload` | any | No | Data for the action |
| `routing` | object | No | P2P/pivoting metadata |
| `meta` | object | No | Extensible metadata |

## Action System

Actions are loose values - any JSON that components can interpret:

```json
"action": "query"
"action": "rpc.call"
"action": "stream.open"
"action": {"name": "custom", "version": 1}
```

### Common Action Categories

| Category | Prefix | Purpose |
|----------|--------|---------|
| Core | `query`, `create`, `delete`, `update` | CRUD operations |
| RPC | `rpc.call`, `rpc.response` | Remote procedure calls |
| Streams | `stream.open`, `stream.data`, `stream.close` | Bidirectional streams |
| Events | `subscribe`, `event` | Event notifications |
| Network | `connect`, `disconnect`, `forward` | Connection handling |

## Payload Formats

Payloads vary by action - components interpret them flexibly:

### RPC Payload

```json
{
    "action": "rpc.call",
    "target": ["endpoints", "ep1", "components", "tcp-client"],
    "payload": {
        "method": "connect",
        "params": {"address": "127.0.0.1", "port": 443}
    }
}
```

```json
{
    "action": "rpc.response",
    "payload": {
        "success": true,
        "result": {"connected": true}
    }
}
```

### Stream Payload

```json
{
    "action": "stream.open",
    "payload": {
        "channel": "stdio",
        "mode": "bidirectional"
    }
}
```

```json
{
    "action": "stream.data",
    "payload": {
        "channel": "stdout",
        "data": "SGVsbG8gd29ybGQ=",  // base64 encoded
        "chunk": 0,
        "total": 1
    }
}
```

```json
{
    "action": "stream.close",
    "payload": {
        "channel": "stdio",
        "reason": "eof"
    }
}
```

### Event Payload

```json
{
    "action": "subscribe",
    "target": ["endpoints", "ep1", "logs"],
    "payload": {
        "event": "new_entry",
        "callback": "components/event-handler"
    }
}
```

```json
{
    "action": "event",
    "source": ["endpoints", "ep1", "logs"],
    "payload": {
        "event": "new_entry",
        "data": "some log message"
    }
}
```

### P2P Routing Payload

```json
{
    "action": "forward",
    "target": ["peers", "peer-2", "endpoints", "ep2"],
    "routing": {
        "via": "peer-1",
        "hop": 1,
        "max_hops": 3
    },
    "payload": {...}
}
```

## Response Pattern

All responses use a generic format:

```json
{
    "id": "req-123",
    "action": "response",
    "source": ["endpoints", "ep1"],
    "payload": {
        "success": true,
        "result": {"connected": true},
        "error": null
    }
}
```

```json
{
    "id": "req-123",
    "action": "response",
    "payload": {
        "success": false,
        "error": {
            "code": 404,
            "message": "not found"
        }
    }
}
```

## Path Format

Paths can be represented in multiple ways:

```json
"target": "components/tcp-client"
"target": ["components", "tcp-client"]
"target": ["endpoints", "ep1", "connections", "conn-1"]
```

## Message Types (Optional Wrapper)

For transport-level distinction:

| Type | Description |
|------|-------------|
| `req` | Request - expecting a response |
| `resp` | Response - reply to a request |
| `event` | Unsolicited notification |
| `stream` | Stream data message |

Example with type wrapper:

```json
{
    "id": "msg-123",
    "type": "req",
    "target": ["components", "tcp-client"],
    "action": "rpc.call",
    "payload": {"method": "connect", "params": {...}}
}
```

## Binary Framing (Optional)

For bandwidth-constrained links:

```
[4 bytes: length][1 byte: flags][id (16 bytes, optional)][payload...]
```

- Length: Big-endian u32
- Flags: 0x01 = compressed, 0x02 = encrypted
- ID: UUID v4 (optional, for correlation)

## Examples

### Full RPC Call

```json
{
    "id": "req-123",
    "source": "server",
    "target": ["endpoints", "ep1", "components", "tcp-client"],
    "action": "rpc.call",
    "payload": {
        "method": "connect",
        "params": {"address": "127.0.0.1", "port": 8080}
    }
}
```

### Response

```json
{
    "id": "resp-123",
    "source": ["endpoints", "ep1"],
    "target": "server",
    "action": "response",
    "payload": {
        "success": true,
        "result": {
            "connected": true,
            "local_addr": "192.168.1.100:12345",
            "remote_addr": "127.0.0.1:8080"
        }
    }
}
```

### Log Subscription + Events

```json
{
    "id": "sub-1",
    "source": "server",
    "target": ["endpoints", "ep1", "logs"],
    "action": "subscribe",
    "payload": {
        "event": "new_entry",
        "callback": ["components", "log-forwarder"]
    }
}
```

```json
{
    "id": "evt-1",
    "source": ["endpoints", "ep1", "logs"],
    "target": ["components", "log-forwarder"],
    "action": "event",
    "payload": {
        "event": "new_entry",
        "data": "2024-01-15 10:30:45 - Connection established"
    }
}
```

### P2P Forward

```json
{
    "id": "req-456",
    "source": "server",
    "target": ["endpoints", "ep2", "components", "shell"],
    "action": "rpc.call",
    "routing": {
        "via": "peer-1",
        "hop": 1,
        "max_hops": 3
    },
    "payload": {
        "method": "execute",
        "params": {"command": "whoami"}
    }
}
```

## Implementation Notes

### Action Matching

```rust
impl TreeMessage {
    /// Check if action matches a pattern
    pub fn action_is(&self, action: &str) -> bool {
        match &self.action {
            Value::String(s) => s == action || s.ends_with(&format!(".{}", action)),
            _ => false,
        }
    }
    
    /// Get method name from RPC payload
    pub fn get_method(&self) -> Option<String> {
        self.payload.get("method")
            .and_then(|m| m.as_str())
            .map(String::from)
    }
    
    /// Get stream channel from payload
    pub fn get_channel(&self) -> Option<String> {
        self.payload.get("channel")
            .and_then(|c| c.as_str())
            .map(String::from)
    }
}
```

### Component Interpretation

Components define their own action handlers:

```rust
impl TreeElement for TcpClient {
    fn send_message(&mut self, target: Value, message: Value) -> Value {
        match message.get("method").and_then(|m| m.as_str()) {
            Some("connect") => self.handle_connect(message),
            Some("send") => self.handle_send(message),
            Some("recv") => self.handle_recv(message),
            _ => json!({"error": "unknown method"}),
        }
    }
}
```

## Comparison to Other Protocols

| Aspect | Tree Protocol | OpenC2 | OST-C2 | Mythic |
|--------|--------------|--------|--------|--------|
| Typing | Loose/JSON | Strict enum | Binary enum | JSON |
| Actions | Namespaced strings | Fixed enum | Type/Code | Action strings |
| RPC | Yes | No | Limited | Yes |
| Streams | Yes | No | No | No |
| Events | Yes | No | No | Yes |
| P2P | Via paths | No | Yes | Via delegates |
| Extensibility | High | Medium | Low | Medium |

## Extensibility Points

1. **New actions**: Add any namespaced string - no code changes needed
2. **Payload structure**: Any JSON - components interpret as needed
3. **Meta field**: Add transport, timing, or custom metadata
4. **Routing field**: Add P2P-specific metadata as needed
5. **Namespacing**: Use prefixes like `openc2.`, `custom.`, `vendor.` for compatibility

## Serialization

- Default: JSON (human-readable, debuggable)
- Optional: CBOR (binary, compact)
- Custom: Any format via `meta.serialization` hint
