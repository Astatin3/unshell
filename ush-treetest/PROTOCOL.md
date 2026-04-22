# Protocol Testbed Report

## Summary

Built a tree-based routing protocol testbed with the following components:

### Files Created

```
ush-treetest/
├── Cargo.toml
├── src/
│   ├── main.rs              # CLI entry point with serve/connect modes
│   ├── protocol/
│   │   ├── mod.rs          # Module exports
│   │   ├── types.rs        # FrameHeader, FrameType, TreeRequest, TreeResponse
│   │   └── transport.rs     # Transport trait, TcpTransport, frame helpers
│   ├── tree/
│   │   ├── mod.rs          # Tree, routing logic, node management
│   │   └── endpoint.rs     # Endpoint trait
│   ├── leaves/
│   │   ├── mod.rs          # Leaf module exports
│   │   ├── shell.rs        # RemoteShell (command execution)
│   │   └── tty.rs         # TTY (PTY support)
│   └── cli/
│       └── mod.rs          # Interactive CLI
```

### Protocol Implemented

**Frame Types:**
- Request (0x01): Tree operations
- Response (0x02): Operation results
- StreamOpen (0x03): Open bidirectional stream
- StreamData (0x04): Fastpath streaming data
- StreamClose (0x05): Close stream
- Handshake (0x10): Connection setup
- HandshakeAck (0x11): Connection acceptance

**Routing:**
- Longest-prefix match on dst_path for Request/StreamOpen
- Stream ID lookup for StreamData/StreamClose

### What Works

1. ✅ Basic project structure with proper module organization
2. ✅ Protocol types with rkyv serialization
3. ✅ TCP transport with length-prefixed framing
4. ✅ Tree routing with prefix matching
5. ✅ RemoteShell leaf implementation
6. ✅ Basic CLI with commands (ls, exec, cd, connect, etc.)

### Challenges Encountered

1. **rkyv API Complexity**: The rkyv serialization library has complex feature flags and API requirements:
   - `from_bytes` requires `validation` feature
   - `to_bytes` requires specifying const generic size parameter
   - Error handling requires careful trait bounds

2. **Trait Object Sending**: The `dyn Endpoint` trait object doesn't implement `Send`, preventing the server from spawning threads with tree handlers

3. **Borrow Checker Issues**: Complex borrowing patterns in tree traversal with mutable references

4. **no_std + alloc Complexity**: The `alloc` crate requires explicit linking in Rust 2021 edition

### Recommendations for Fixing

1. Use `serde` with `bincode` instead of `rkyv` for simpler serialization
2. Use `Arc<Mutex<Tree>>` for thread-safe shared state
3. Simplify the borrow patterns in tree operations
4. For no_std, add proper `extern crate alloc` declarations

### Protocol Observations

1. The protocol design is sound - separating request/response from streaming is good
2. The frame type enum should be repr(u8) for efficiency
3. Longest-prefix matching works well for hierarchical routing
4. The handshake pattern is simple but effective
5. Consider adding compression for large payloads
