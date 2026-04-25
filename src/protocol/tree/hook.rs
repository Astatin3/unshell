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
    pub local_ended: bool,
    pub peer_ended: bool,
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

/// Duplicate hook insertion error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HookConflict;

/// Durable hook state tables.
#[derive(Debug)]
pub struct HookTable {
    pending: BTreeMap<u64, BTreeMap<Vec<String>, PendingHook>>,
    active: BTreeMap<u64, BTreeMap<Vec<String>, ActiveHook>>,
    active_by_peer: BTreeMap<u64, BTreeMap<Vec<String>, Vec<String>>>,
    next_id: u64,
}

impl Default for HookTable {
    fn default() -> Self {
        Self {
            pending: BTreeMap::new(),
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

    /// Inserts a pending hook created by a received call.
    pub fn insert_pending(&mut self, pending: PendingHook) -> Result<(), HookConflict> {
        if self.pending(&HookKey::new(pending.return_path.clone(), pending.hook_id)).is_some()
            || self.active(&HookKey::new(pending.return_path.clone(), pending.hook_id)).is_some()
        {
            return Err(HookConflict);
        }

        self.pending
            .entry(pending.hook_id)
            .or_default()
            .insert(pending.return_path.clone(), pending);
        Ok(())
    }

    /// Inserts an already-active hook flow.
    pub fn insert_active(&mut self, active: ActiveHook) -> Result<(), HookConflict> {
        let key = HookKey::new(active.return_path.clone(), active.hook_id);
        if self.pending(&key).is_some() || self.active(&key).is_some() {
            return Err(HookConflict);
        }

        self.active_by_peer
            .entry(active.hook_id)
            .or_default()
            .insert(active.peer_path.clone(), active.return_path.clone());
        self.active
            .entry(active.hook_id)
            .or_default()
            .insert(active.return_path.clone(), active);
        Ok(())
    }

    /// Promotes one pending hook into active state after local acceptance.
    pub fn activate_pending(&mut self, key: &HookKey) -> Option<()> {
        let pending = self.remove_pending(key)?;
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

    /// Removes a pending hook entry.
    pub fn remove_pending(&mut self, key: &HookKey) -> Option<PendingHook> {
        let hooks = self.pending.get_mut(&key.hook_id)?;
        let pending = hooks.remove(key.return_path.as_slice())?;
        if hooks.is_empty() {
            self.pending.remove(&key.hook_id);
        }
        Some(pending)
    }

    /// Removes an active hook entry.
    pub fn remove_active(&mut self, key: &HookKey) -> Option<ActiveHook> {
        let hooks = self.active.get_mut(&key.hook_id)?;
        let active = hooks.remove(key.return_path.as_slice())?;
        if hooks.is_empty() {
            self.active.remove(&key.hook_id);
        }

        if let Some(peer_index) = self.active_by_peer.get_mut(&key.hook_id) {
            peer_index.remove(active.peer_path.as_slice());
            if peer_index.is_empty() {
                self.active_by_peer.remove(&key.hook_id);
            }
        }
        Some(active)
    }

    /// Returns a pending hook by its host-scoped key.
    #[must_use]
    pub fn pending(&self, key: &HookKey) -> Option<&PendingHook> {
        self.pending
            .get(&key.hook_id)?
            .get(key.return_path.as_slice())
    }

    /// Returns an active hook by its host-scoped key.
    #[must_use]
    pub fn active(&self, key: &HookKey) -> Option<&ActiveHook> {
        self.active.get(&key.hook_id)?.get(key.return_path.as_slice())
    }

    /// Returns mutable access to an active hook by its host-scoped key.
    pub fn active_mut(&mut self, key: &HookKey) -> Option<&mut ActiveHook> {
        self.active
            .get_mut(&key.hook_id)?
            .get_mut(key.return_path.as_slice())
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
        if self.active(&host_key).is_some() {
            return Some(host_key);
        }

        self.active_by_peer
            .get(&hook_id)?
            .get(peer_path)
            .cloned()
            .map(|return_path| HookKey::new(return_path, hook_id))
    }

    /// Marks one locally-originated final data packet.
    pub fn mark_local_end(&mut self, key: &HookKey) -> bool {
        let Some(active) = self.active_mut(key) else {
            return false;
        };
        active.local_ended = true;
        active.peer_ended
    }

    /// Marks one peer-originated final data packet.
    pub fn mark_peer_end(&mut self, key: &HookKey) -> bool {
        let Some(active) = self.active_mut(key) else {
            return false;
        };
        active.peer_ended = true;
        active.local_ended
    }

    /// Returns whether one key still has pending or active state.
    #[must_use]
    pub fn contains(&self, key: &HookKey) -> bool {
        self.pending(key).is_some() || self.active(key).is_some()
    }

    /// Returns the number of active hooks.
    #[must_use]
    pub fn active_len(&self) -> usize {
        self.active.values().map(BTreeMap::len).sum()
    }
}
