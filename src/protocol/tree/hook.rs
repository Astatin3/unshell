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
    pub caller_src_path: Vec<String>,
    pub return_path: Vec<String>,
    pub hook_id: u64,
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
    pub peer_finished: bool,
}

/// Durable hook state tables.
#[derive(Debug, Default)]
pub struct HookTable {
    pending: BTreeMap<HookKey, PendingHook>,
    active: BTreeMap<HookKey, ActiveHook>,
}

impl HookTable {
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

    pub fn insert_pending(&mut self, pending: PendingHook) {
        // WARNING: hook tables intentionally own their path and procedure strings.
        // Hook state must outlive any individual frame buffer.
        let key = HookKey::new(pending.return_path.clone(), pending.hook_id);
        self.pending.insert(key, pending);
    }

    pub fn insert_active(&mut self, active: ActiveHook) {
        let key = HookKey::new(active.return_path.clone(), active.hook_id);
        self.active.insert(key, active);
    }

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

    pub fn remove_pending(&mut self, key: &HookKey) -> Option<PendingHook> {
        self.pending.remove(key)
    }

    pub fn remove_active(&mut self, key: &HookKey) -> Option<ActiveHook> {
        self.active.remove(key)
    }

    pub fn pending(&self, key: &HookKey) -> Option<&PendingHook> {
        self.pending.get(key)
    }

    pub fn active(&self, key: &HookKey) -> Option<&ActiveHook> {
        self.active.get(key)
    }

    pub fn active_mut(&mut self, key: &HookKey) -> Option<&mut ActiveHook> {
        self.active.get_mut(key)
    }

    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    pub fn active_len(&self) -> usize {
        self.active.len()
    }
}
