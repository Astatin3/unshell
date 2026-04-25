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

/// Duplicate hook insertion error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HookConflict;

/// Durable hook state tables.
#[derive(Debug)]
pub struct HookTable {
    pending: BTreeMap<HookKey, PendingHook>,
    active: BTreeMap<HookKey, ActiveHook>,
    next_id: u64,
}

impl Default for HookTable {
    fn default() -> Self {
        Self {
            pending: BTreeMap::new(),
            active: BTreeMap::new(),
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

    /// Inserts a pending hook created by an inbound call.
    pub fn insert_pending(&mut self, pending: PendingHook) -> Result<(), HookConflict> {
        let key = HookKey::new(pending.return_path.clone(), pending.hook_id);
        if self.pending.contains_key(&key) || self.active.contains_key(&key) {
            return Err(HookConflict);
        }
        self.pending.insert(key, pending);
        Ok(())
    }

    /// Inserts an already-active hook flow.
    pub fn insert_active(&mut self, active: ActiveHook) -> Result<(), HookConflict> {
        let key = HookKey::new(active.return_path.clone(), active.hook_id);
        if self.pending.contains_key(&key) || self.active.contains_key(&key) {
            return Err(HookConflict);
        }
        self.active.insert(key, active);
        Ok(())
    }

    /// Promotes a pending hook into the active table once its peer is known.
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

    /// Removes a pending hook entry.
    pub fn remove_pending(&mut self, key: &HookKey) -> Option<PendingHook> {
        self.pending.remove(key)
    }

    /// Removes an active hook entry.
    pub fn remove_active(&mut self, key: &HookKey) -> Option<ActiveHook> {
        self.active.remove(key)
    }

    /// Returns a pending hook by its host-scoped key.
    #[must_use]
    pub fn pending(&self, key: &HookKey) -> Option<&PendingHook> {
        self.pending.get(key)
    }

    /// Returns an active hook by its host-scoped key.
    #[must_use]
    pub fn active(&self, key: &HookKey) -> Option<&ActiveHook> {
        self.active.get(key)
    }

    /// Returns mutable access to an active hook by its host-scoped key.
    pub fn active_mut(&mut self, key: &HookKey) -> Option<&mut ActiveHook> {
        self.active.get_mut(key)
    }

    /// Finds an active hook key for a non-host peer receiving continued data.
    ///
    /// Rationale: `hook_id` is scoped to the hook host, so a subordinate peer
    /// cannot derive the full key from the packet header alone. The peer uses
    /// its already-validated active state to recover the host-scoped key.
    pub fn find_active_key_by_peer(&self, hook_id: u64, peer_path: &[String]) -> Option<HookKey> {
        let mut matching_keys = self
            .active
            .iter()
            .filter(|(_key, active)| active.hook_id == hook_id && active.peer_path == peer_path)
            .map(|(key, _)| key.clone());

        let key = matching_keys.next()?;
        if matching_keys.next().is_some() {
            return None;
        }
        Some(key)
    }

    /// Returns the number of pending hooks.
    #[must_use]
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    /// Returns the number of active hooks.
    #[must_use]
    pub fn active_len(&self) -> usize {
        self.active.len()
    }
}
