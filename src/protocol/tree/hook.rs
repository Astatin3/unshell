//! Hook state for pending and active protocol flows.

use alloc::{collections::BTreeMap, string::String, vec::Vec};

/// Hook table key scoped to the hook host path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct HookKey {
    /// Path of the endpoint hosting the hook.
    pub return_path: Vec<String>,
    /// Hook identifier scoped to `return_path`.
    pub hook_id: u64,
}

impl HookKey {
    /// Creates a host-scoped key from the return path and hook identifier.
    #[must_use]
    pub fn new(return_path: Vec<String>, hook_id: u64) -> Self {
        Self {
            return_path,
            hook_id,
        }
    }
}

/// Active hook context used for ordinary data traffic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveHook {
    pub return_path: Vec<String>,
    pub hook_id: u64,
    pub peer_path: Vec<String>,
    pub procedure_id: String,
    pub dst_leaf: Option<String>,
}

/// Peer-scoped index key used by the non-host side of a hook.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PeerHookKey {
    hook_id: u64,
    peer_path: Vec<String>,
}

/// Duplicate hook insertion error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HookConflict;

/// Durable hook state tables.
#[derive(Debug)]
pub struct HookTable {
    active: BTreeMap<HookKey, ActiveHook>,
    active_by_peer: BTreeMap<PeerHookKey, HookKey>,
    next_id: u64,
}

impl Default for HookTable {
    fn default() -> Self {
        Self {
            active: BTreeMap::new(),
            active_by_peer: BTreeMap::new(),
            next_id: 1,
        }
    }
}

impl HookTable {
    /// Allocates the next locally unique hook identifier.
    ///
    /// Hook IDs are scoped by return path, so this counter only needs to be
    /// unique within one endpoint runtime.
    #[must_use]
    pub fn allocate_hook_id(&mut self, _return_path: &[String]) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        id
    }

    /// Inserts an already-active hook flow.
    pub fn insert_active(&mut self, active: ActiveHook) -> Result<(), HookConflict> {
        let key = HookKey::new(active.return_path.clone(), active.hook_id);
        let peer_key = PeerHookKey {
            hook_id: active.hook_id,
            peer_path: active.peer_path.clone(),
        };
        if self.active.contains_key(&key) || self.active_by_peer.contains_key(&peer_key) {
            return Err(HookConflict);
        }
        self.active_by_peer.insert(peer_key, key.clone());
        self.active.insert(key, active);
        Ok(())
    }

    /// Removes an active hook entry.
    pub fn remove_active(&mut self, key: &HookKey) -> Option<ActiveHook> {
        let active = self.active.remove(key)?;
        self.active_by_peer.remove(&PeerHookKey {
            hook_id: active.hook_id,
            peer_path: active.peer_path.clone(),
        });
        Some(active)
    }

    /// Returns an active hook by its host-scoped key.
    #[must_use]
    pub fn active(&self, key: &HookKey) -> Option<&ActiveHook> {
        self.active.get(key)
    }

    /// Resolves one active hook key from either the host side or the peer side.
    #[must_use]
    pub fn resolve_active_key(
        &self,
        return_path: &[String],
        hook_id: u64,
        peer_path: &[String],
    ) -> Option<HookKey> {
        let host_key = HookKey::new(return_path.to_vec(), hook_id);
        if self.active.contains_key(&host_key) {
            return Some(host_key);
        }

        self.active_by_peer
            .get(&PeerHookKey {
                hook_id,
                peer_path: peer_path.to_vec(),
            })
            .cloned()
    }

    /// Returns the number of active hooks.
    #[must_use]
    pub fn active_len(&self) -> usize {
        self.active.len()
    }
}
