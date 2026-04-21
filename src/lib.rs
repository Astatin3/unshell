//! # UnShell Core Library
//!
//! This crate provides the core building blocks for the UnShell C2 framework:
//!
//! - **[`protocol`]** — wire types: `PacketHeader`, `TreeRequest`, `TreeResponse`,
//!   `HandshakeMessage`, `HandshakeAck`, and associated enums.
//! - **[`transport`]** — the `Transport` trait and its TCP implementation.
//! - **[`tree`]** — the `Tree` and `Endpoint` abstractions for module dispatch.
//! - **[`logger`]** — lightweight logging (no dependency on `std::io`).
//!
//! ## `no_std` Compatibility
//!
//! This crate is `no_std` but requires `alloc`. It can be used in the payload
//! binary which runs without a full standard library.
//!
//! Binaries that have `std` available (the router, the CLI) can also use this
//! crate; they simply get `alloc` types backed by the system allocator.
//!
//! ## Architecture
//!
//! ```text
//! ┌────────────────────────────────────────────────────────────────┐
//! │                    Router / Relay                               │
//! │  Reads PacketHeader → longest-prefix routes to node            │
//! │  Payload bytes forwarded opaque                                 │
//! └───────────┬─────────────────────────┬──────────────────────────┘
//!             │ TCP                     │ TCP
//!    ┌────────▼────────┐      ┌─────────▼──────────────────────────┐
//!    │  Operator Node  │      │  Payload Node(s)                    │
//!    │  (ush-cli)      │      │  Local Tree + Endpoint modules      │
//!    │  Interactive    │      │  Reverse-connects to router         │
//!    │  REPL           │      │  Recv loop → dispatch → respond     │
//!    └─────────────────┘      └─────────────────────────────────────┘
//! ```
//!
//! For the full protocol specification, see `PROTOCOL.md` in the repository root.

// Enable std when the `tcp` feature is active (TCP transport requires it).
// Without tcp, we stay fully no_std for bare-metal payload targets.
#![cfg_attr(not(feature = "tcp"), no_std)]
// no_main is only applied in non-test builds.
// The test harness generates its own main function, so we must NOT suppress it.
#![cfg_attr(not(test), no_main)]

extern crate alloc;

pub mod logger;
pub mod protocol;
pub mod transport;
pub mod tree;

// Re-export the obfuscation crate so payloads only need to depend on `unshell`.
pub use ush_obfuscate as obfuscate;
