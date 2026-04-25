//! Hook state for pending and active protocol flows.

use alloc::{collections::BTreeMap, string::String, vec::Vec};

/// Hook table key scoped to the hook host path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct HookKey {
    pub return_path: Vec<String>,
    pub hook_id: u64,
}

impl HookKey {
    #[must_use]
    pub fn new(return_path: Vec<String>, hook_id: u64) -> Self {
        Self {
            return_path,
            hook_id,
        }
    }
}

/// Pending hook context used only for fault attribution before activation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingHook {
    pub return_path: Vec<String>,
    pub hook_id: u64,
    pub caller_src_path: Vec<String>,
    pub procedure_id: String,
    pub dst_leaf: Option<String>,
}

/// Active hook context used for ordinary data traffic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveHook {
    pub return_path: Vec<String>,
    pub hook_id: u64,
    pub peer_path: Vec<String>,
    pub procedure_id: String,
    pub dst_leaf: Option<String>,
    pub local_ended: bool,
    pub peer_ended: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PeerHookKey {
    hook_id: u64,
    peer_path: Vec<String>,
}

/// Duplicate hook insertion error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HookConflict;

/// Durable hook state tables.
#[derive(Debug, Default)]
pub struct HookTable {
    pending: BTreeMap<HookKey, PendingHook>,
    active: BTreeMap<HookKey, ActiveHook>,
    active_by_peer: BTreeMap<PeerHookKey, HookKey>,
    next_id: u64,
}

impl HookTable {
    #[must_use]
    pub fn allocate_hook_id(&mut self, _return_path: &[String]) -> u64 {
        let id = self.next_id.max(1);
        self.next_id = id.wrapping_add(1);
        id
    }

    pub fn insert_pending(&mut self, pending: PendingHook) -> Result<(), HookConflict> {
        let key = HookKey::new(pending.return_path.clone(), pending.hook_id);
        if self.pending.contains_key(&key) || self.active.contains_key(&key) {
            return Err(HookConflict);
        }
        self.pending.insert(key, pending);
        Ok(())
    }

    pub fn activate_pending(&mut self, key: &HookKey) -> Option<()> {
        let pending = self.pending.remove(key)?;
        self.insert_active(ActiveHook {
            return_path: pending.return_path,
            hook_id: pending.hook_id,
            peer_path: pending.caller_src_path,
            procedure_id: pending.procedure_id,
            dst_leaf: pending.dst_leaf,
            local_ended: false,
            peer_ended: false,
        })
        .ok()?;
        Some(())
    }

    pub fn insert_active(&mut self, active: ActiveHook) -> Result<(), HookConflict> {
        let key = HookKey::new(active.return_path.clone(), active.hook_id);
        let peer_key = PeerHookKey {
            hook_id: active.hook_id,
            peer_path: active.peer_path.clone(),
        };
        if self.pending.contains_key(&key)
            || self.active.contains_key(&key)
            || self.active_by_peer.contains_key(&peer_key)
        {
            return Err(HookConflict);
        }
        self.active_by_peer.insert(peer_key, key.clone());
        self.active.insert(key, active);
        Ok(())
    }

    pub fn remove_pending(&mut self, key: &HookKey) -> Option<PendingHook> {
        self.pending.remove(key)
    }

    pub fn remove_active(&mut self, key: &HookKey) -> Option<ActiveHook> {
        let active = self.active.remove(key)?;
        self.active_by_peer.remove(&PeerHookKey {
            hook_id: active.hook_id,
            peer_path: active.peer_path.clone(),
        });
        Some(active)
    }

    #[must_use]
    pub fn pending(&self, key: &HookKey) -> Option<&PendingHook> {
        self.pending.get(key)
    }

    #[must_use]
    pub fn active(&self, key: &HookKey) -> Option<&ActiveHook> {
        self.active.get(key)
    }

    pub fn active_mut(&mut self, key: &HookKey) -> Option<&mut ActiveHook> {
        self.active.get_mut(key)
    }

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

    pub fn mark_local_end(&mut self, key: &HookKey) -> bool {
        let Some(active) = self.active_mut(key) else {
            return false;
        };
        active.local_ended = true;
        active.peer_ended
    }

    pub fn mark_peer_end(&mut self, key: &HookKey) -> bool {
        let Some(active) = self.active_mut(key) else {
            return false;
        };
        active.peer_ended = true;
        active.local_ended
    }

    #[must_use]
    pub fn active_len(&self) -> usize {
        self.active.len()
    }

    #[must_use]
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }
}
