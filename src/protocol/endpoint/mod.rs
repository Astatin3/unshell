mod connections;
mod hook_output;
mod hooks;
mod queues;
mod routing;

pub use hooks::HookID;

use alloc::vec::Vec;

use crate::{
    crypto::Counter,
    protocol::{ConnectionSet, HookMap, Path, RouteMap},
};

/// Local routing state for one protocol node.
///
/// `Endpoint` deliberately owns only route, hook, and connection tables. Leaves are
/// caller-owned concrete values, which keeps small firmware-style binaries from
/// linking dynamic leaf registries or boxed trait objects.
pub struct Endpoint {
    /// This endpoint's identifier.
    pub id: u32,

    /// Counter used to allocate locally unique hook ids.
    pub(crate) last_hook: Counter,

    /// Absolute path for this node. An empty path means routing is not initialized.
    pub path: Path,

    /// Adjacent endpoints and whether each adjacent endpoint is upstream/authority.
    pub connections: ConnectionSet,

    /// Active hook id to adjacent peer mappings.
    pub(crate) hooks: HookMap,

    /// Packets delivered locally and waiting for leaf consumption.
    pub(crate) inbound: RouteMap,

    /// Packets queued for adjacent endpoints and waiting for transport leaves.
    pub(crate) outbound: RouteMap,
}

impl Endpoint {
    /// Creates endpoint routing state for one protocol node.
    ///
    /// Leaves are intentionally owned by the caller instead of stored behind
    /// endpoint-local trait objects. That keeps minimized binaries from pulling in
    /// dynamic dispatch and allocation paths when a firmware-style application uses a
    /// fixed set of concrete leaves.
    pub fn new(id: u32) -> Self {
        Self {
            id,
            // Init the hook at 0, which will increment
            last_hook: Counter::new(),

            // Set the current path as an empty vec
            path: Vec::new(),
            hooks: Vec::new(),
            connections: Vec::new(),
            inbound: Vec::new(),
            outbound: Vec::new(),
        }
    }
}
