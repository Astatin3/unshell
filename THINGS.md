### Binary
- Obfustcation
- Randomly generated packed binaries
- Rust is already hard to decompile?
- Persistance
  - Probably out of scope
- Build targets
  - To achieve a minimal size, there should probably be a way to pack diffrent features with the actual result binary.
  - There should also be a way to update one of the hosts with the new functionality.

### Network
- Diffrent traffic obfuscators:
  - ICMP
  - HTTPS (Using actual webpages)
  - OpenVPN (Hard to replicate in rust)
- "Hole Widening"
  - Initial reverse shell is the final one
  - Minimal presence on remote machine
  - Instead of downloading binaries and then executing them, use the shell connection as a kind of remote storage server.
- Pivoting
  - UI for sub-connections.
  - A protocol that acts similar to routers and DHCP, registering known devices with the C2 server. Sub-devices will relay packets
  - Packets must be encrypted, so that only the destination can decrypt.
    - How?
- ### Encryption
  - Diffrent "encryptors" such as PGP
  - Everything must be self-implemented because of traffic monitors such as mitmproxy
  - HTTPS could transmit over the actual TLS implemented by the system, and transfer data through things such as base64 images on webpages, which would itself be encrypted

### UI
- Egui??
  - Usable both on web and on-device
- Network diagram creation tool

### Tools
- These are the diffrent tools that can be transmitted, and then run on a machine
- Host discovery && port scanning
- File upload and download
- Screenshare
- Virtual browser and desktop
- meterpreter functionality?
- Scripting?
