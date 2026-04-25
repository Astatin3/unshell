//! Protocol implementation traits exposed by the core crate.
//!
//! These traits collect the core contracts needed to plug framing, routing,
//! hook storage, leaf metadata, and packet processing into an implementation.

use alloc::{string::String, vec::Vec};

use super::{
    FrameBytes, FrameCodec, LeafIntrospection, LeafIntrospectionSummary,
    tree::{
        ActiveHook, Endpoint, EndpointError, EndpointOutcome, HookConflict, HookKey, HookTable,
        Ingress, LeafNode, LeafSpec, PendingHook, RouteProvider,
    },
};

/// Packet framing contract for the canonical wire format.
pub trait PacketFraming: FrameCodec {}

impl<T> PacketFraming for T where T: FrameCodec + ?Sized {}

/// Route resolution contract for endpoint path delivery.
pub trait RouteResolution: RouteProvider {}

impl<T> RouteResolution for T where T: RouteProvider + ?Sized {}

/// Hook storage contract for pending and active protocol flows.
pub trait HookStore {
    /// Allocates a hook identifier scoped to `return_path`.
    fn allocate_hook_id(&mut self, return_path: &[String]) -> u64;

    /// Inserts a hook created by an incoming call before the peer is confirmed.
    fn insert_pending(&mut self, pending: PendingHook) -> Result<(), HookConflict>;

    /// Inserts an already-established hook flow.
    fn insert_active(&mut self, active: ActiveHook) -> Result<(), HookConflict>;

    /// Promotes a pending hook once the responding peer is known.
    fn activate_pending(&mut self, key: &HookKey, peer_path: Vec<String>) -> Option<()>;

    /// Removes a pending hook.
    fn remove_pending(&mut self, key: &HookKey) -> Option<PendingHook>;

    /// Removes an active hook.
    fn remove_active(&mut self, key: &HookKey) -> Option<ActiveHook>;

    /// Returns immutable access to a pending hook.
    fn pending(&self, key: &HookKey) -> Option<&PendingHook>;

    /// Returns immutable access to an active hook.
    fn active(&self, key: &HookKey) -> Option<&ActiveHook>;

    /// Returns mutable access to an active hook.
    fn active_mut(&mut self, key: &HookKey) -> Option<&mut ActiveHook>;
}

impl HookStore for HookTable {
    fn allocate_hook_id(&mut self, return_path: &[String]) -> u64 {
        HookTable::allocate_hook_id(self, return_path)
    }

    fn insert_pending(&mut self, pending: PendingHook) -> Result<(), HookConflict> {
        HookTable::insert_pending(self, pending)
    }

    fn insert_active(&mut self, active: ActiveHook) -> Result<(), HookConflict> {
        HookTable::insert_active(self, active)
    }

    fn activate_pending(&mut self, key: &HookKey, peer_path: Vec<String>) -> Option<()> {
        HookTable::activate_pending(self, key, peer_path)
    }

    fn remove_pending(&mut self, key: &HookKey) -> Option<PendingHook> {
        HookTable::remove_pending(self, key)
    }

    fn remove_active(&mut self, key: &HookKey) -> Option<ActiveHook> {
        HookTable::remove_active(self, key)
    }

    fn pending(&self, key: &HookKey) -> Option<&PendingHook> {
        HookTable::pending(self, key)
    }

    fn active(&self, key: &HookKey) -> Option<&ActiveHook> {
        HookTable::active(self, key)
    }

    fn active_mut(&mut self, key: &HookKey) -> Option<&mut ActiveHook> {
        HookTable::active_mut(self, key)
    }
}

/// Leaf metadata contract used for protocol discovery payloads.
pub trait LeafMetadata {
    /// Returns the leaf name exposed in routing and introspection.
    fn leaf_name(&self) -> &str;

    /// Returns the supported canonical procedure identifiers.
    fn procedures(&self) -> &[String];

    /// Builds the compact endpoint-wide discovery record for this leaf.
    fn summary(&self) -> LeafIntrospectionSummary {
        LeafIntrospectionSummary {
            leaf_name: self.leaf_name().into(),
            procedures: self.procedures().to_vec(),
        }
    }

    /// Builds the full leaf-specific discovery payload.
    fn introspection(&self) -> LeafIntrospection {
        LeafIntrospection {
            leaf_name: self.leaf_name().into(),
            procedures: self.procedures().to_vec(),
        }
    }
}

impl LeafMetadata for LeafSpec {
    fn leaf_name(&self) -> &str {
        &self.name
    }

    fn procedures(&self) -> &[String] {
        &self.procedures
    }
}

impl LeafMetadata for LeafNode {
    fn leaf_name(&self) -> &str {
        &self.name
    }

    fn procedures(&self) -> &[String] {
        &self.procedures
    }
}

/// Packet processor and local runtime contract for framed protocol traffic.
pub trait PacketProcessor {
    /// Returns the endpoint path that owns this processor.
    fn path(&self) -> &[String];

    /// Receives one framed packet from the given ingress side.
    fn receive(
        &mut self,
        ingress: &Ingress,
        frame: FrameBytes,
    ) -> Result<EndpointOutcome, EndpointError>;
}

impl<T> PacketProcessor for T
where
    T: Endpoint + ?Sized,
{
    fn path(&self) -> &[String] {
        Endpoint::path(self)
    }

    fn receive(
        &mut self,
        ingress: &Ingress,
        frame: FrameBytes,
    ) -> Result<EndpointOutcome, EndpointError> {
        Endpoint::receive(self, ingress, frame)
    }
}
