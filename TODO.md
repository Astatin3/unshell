### Functionality
- Add 'signals' interface between modules
- Write compilation helper CLI for building payload and breakout module
- Make CLI
- Make GUI

### Topology
- Move server and client components into their own cargo projects
- Write wire protocol spec: `PROTOCOL.md` or doc comment in the protocol module. Spec the two-part frame format `[u32 header_len][rkyv PacketHeader][u32 payload_len][rkyv payload]` with `PacketHeader { dst_path, src_path, packet_type }`. Required before router and payload implementations can be written independently without diverging. See design doc: ~/.gstack/projects/astatin3-unshell/astatin3-main-design-20260420-223152.md

### Obfuscation
- Implement custom ELF loading possibly using 'https://github.com/weizhiao/rust-dlopen'
- Macro-based automatic control flow obfuscation
