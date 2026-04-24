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
    /// Creates a new hook key.
    pub fn new(return_path: Vec<String>, hook_id: u64) -> Self {
        Self {
            return_path,
            hook_id,
        }
    }
}

/// Pending hook context created by a received call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingHook {
    /// Original caller path.
    pub caller_src_path: Vec<String>,
    /// Hook host path.
    pub return_path: Vec<String>,
    /// Hook identifier.
    pub hook_id: u64,
    /// Procedure anchored to the call.
    pub procedure_id: String,
    /// Destination leaf from the call.
    pub dst_leaf: Option<String>,
}

/// Active hook context used for ordinary data traffic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveHook {
    /// Path of the endpoint hosting the hook.
    pub return_path: Vec<String>,
    /// Hook identifier.
    pub hook_id: u64,
    /// Expected direct peer for hook traffic.
    pub peer_path: Vec<String>,
    /// Procedure bound to the hook.
    pub procedure_id: String,
    /// Original destination leaf.
    pub dst_leaf: Option<String>,
    /// Whether the peer has indicated completion.
    pub peer_finished: bool,
}

/// Durable hook state tables.
#[derive(Debug, Default)]
pub struct HookTable {
    pending: BTreeMap<HookKey, PendingHook>,
    active: BTreeMap<HookKey, ActiveHook>,
}

impl HookTable {
    /// Allocates the lowest inactive hook id for a return path.
    pub fn allocate_hook_id(&self, return_path: &[String]) -> u64 {
        let mut hook_id = 0u64;
        loop {
            let key = HookKey::new(return_path.to_vec(), hook_id);
            if !self.pending.contains_key(&key) && !self.active.contains_key(&key) {
                return hook_id;
            }
            hook_id = hook_id.saturating_add(1);
        }
    }

    /// Inserts pending hook state.
    pub fn insert_pending(&mut self, pending: PendingHook) {
        // WARNING: hook tables intentionally own their path and procedure strings.
        // Hook state must outlive any individual frame buffer, so borrowing framed
        // transport memory here would be unsound.
        let key = HookKey::new(pending.return_path.clone(), pending.hook_id);
        self.pending.insert(key, pending);
    }

    /// Inserts active hook state.
    pub fn insert_active(&mut self, active: ActiveHook) {
        let key = HookKey::new(active.return_path.clone(), active.hook_id);
        self.active.insert(key, active);
    }

    /// Promotes pending hook state to active state.
    pub fn activate_pending(&mut self, key: &HookKey, peer_path: Vec<String>) -> Option<()> {
        let pending = self.pending.remove(key)?;
        self.active.insert(
            key.clone(),
            ActiveHook {
                return_path: pending.return_path,
                hook_id: pending.hook_id,
                peer_path,
                procedure_id: pending.procedure_id,
                dst_leaf: pending.dst_leaf,
                peer_finished: false,
            },
        );
        Some(())
    }

    /// Removes pending state.
    pub fn remove_pending(&mut self, key: &HookKey) -> Option<PendingHook> {
        self.pending.remove(key)
    }

    /// Removes active state.
    pub fn remove_active(&mut self, key: &HookKey) -> Option<ActiveHook> {
        self.active.remove(key)
    }

    /// Returns pending state.
    pub fn pending(&self, key: &HookKey) -> Option<&PendingHook> {
        self.pending.get(key)
    }

    /// Returns active state.
    pub fn active(&self, key: &HookKey) -> Option<&ActiveHook> {
        self.active.get(key)
    }

    /// Returns mutable active state.
    pub fn active_mut(&mut self, key: &HookKey) -> Option<&mut ActiveHook> {
        self.active.get_mut(key)
    }

    /// Returns the number of pending hooks.
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    /// Returns the number of active hooks.
    pub fn active_len(&self) -> usize {
        self.active.len()
    }
}
