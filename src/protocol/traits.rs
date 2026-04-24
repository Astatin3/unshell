//! Protocol implementation traits exposed by the core crate.
//!
//! These traits collect the core contracts needed to plug framing, routing,
//! hook storage, leaf metadata, and packet processing into an implementation.

use alloc::{string::String, vec::Vec};

use super::{
    FrameBytes, FrameCodec, LeafIntrospection, LeafIntrospectionSummary,
    tree::{
        ActiveHook, Endpoint, EndpointError, EndpointOutcome, HookKey, HookTable, Ingress,
        LeafNode, LeafSpec, PendingHook, RouteProvider,
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
    fn allocate_hook_id(&self, return_path: &[String]) -> u64;
    fn insert_pending(&mut self, pending: PendingHook);
    fn insert_active(&mut self, active: ActiveHook);
    fn activate_pending(&mut self, key: &HookKey, peer_path: Vec<String>) -> Option<()>;
    fn remove_pending(&mut self, key: &HookKey) -> Option<PendingHook>;
    fn remove_active(&mut self, key: &HookKey) -> Option<ActiveHook>;
    fn pending(&self, key: &HookKey) -> Option<&PendingHook>;
    fn active(&self, key: &HookKey) -> Option<&ActiveHook>;
    fn active_mut(&mut self, key: &HookKey) -> Option<&mut ActiveHook>;
}

impl HookStore for HookTable {
    fn allocate_hook_id(&self, return_path: &[String]) -> u64 {
        HookTable::allocate_hook_id(self, return_path)
    }

    fn insert_pending(&mut self, pending: PendingHook) {
        HookTable::insert_pending(self, pending);
    }

    fn insert_active(&mut self, active: ActiveHook) {
        HookTable::insert_active(self, active);
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
    fn leaf_name(&self) -> &str;
    fn procedures(&self) -> &[String];

    fn summary(&self) -> LeafIntrospectionSummary {
        LeafIntrospectionSummary {
            leaf_name: self.leaf_name().into(),
            procedures: self.procedures().to_vec(),
        }
    }

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
    fn path(&self) -> &[String];
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
