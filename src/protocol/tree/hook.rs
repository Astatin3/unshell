//! Hook state for pending and active protocol flows.
//!
//! Hooks move through two phases:
//! - `PendingHook` tracks enough context to attribute faults before the callee accepts.
//! - `ActiveHook` tracks the live bidirectional flow after activation.
//!
//! The table indexes active hooks both by their host-side return path and by the remote
//! peer path so routing code can resolve whichever side of the relationship it currently has.
//! The `HookKey` already carries the host path and hook id, so the pending/active records only
//! store the extra state that actually changes across the hook lifecycle.

use alloc::{collections::BTreeMap, string::String, vec::Vec};

/// Hook table key scoped to the hook host path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct HookKey {
    /// Path of the endpoint hosting the hook state.
    pub return_path: Vec<String>,
    /// Per-host hook identifier.
    pub hook_id: u64,
}

impl HookKey {
    /// Builds the canonical key for a hook hosted at `return_path`.
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
    /// Caller path to promote into `peer_path` once the hook becomes active.
    pub caller_src_path: Vec<String>,
    /// Procedure that created the hook.
    pub procedure_id: String,
    /// Set once the local side has already emitted its terminal message before activation.
    pub local_ended: bool,
}

/// Active hook context used for ordinary data traffic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveHook {
    /// Remote endpoint path currently paired with this hook.
    pub peer_path: Vec<String>,
    /// Procedure that owns the hook conversation.
    pub procedure_id: String,
    /// Set once the local side has emitted its terminal message.
    pub local_ended: bool,
    /// Set once the peer side has emitted its terminal message.
    pub peer_ended: bool,
}

/// Duplicate hook insertion error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HookConflict;

/// Durable hook state tables.
#[derive(Debug, Default)]
pub struct HookTable {
    pending: BTreeMap<HookKey, PendingHook>,
    active: BTreeMap<HookKey, ActiveHook>,
    active_by_peer: BTreeMap<u64, BTreeMap<Vec<String>, HookKey>>,
    next_id: u64,
}

impl HookTable {
    /// Allocates a non-zero hook id for a hook hosted at `return_path`.
    ///
    /// Hook ids are scoped by host path, so this only needs to guarantee uniqueness within the
    /// local table. The wrapped increment keeps allocation infallible for long-lived runtimes.
    #[must_use]
    pub fn allocate_hook_id(&mut self, _return_path: &[String]) -> u64 {
        let id = self.next_id.max(1);
        self.next_id = id.wrapping_add(1);
        id
    }

    /// Inserts a hook that has been announced but not yet accepted by the callee.
    pub fn insert_pending(
        &mut self,
        key: HookKey,
        pending: PendingHook,
    ) -> Result<(), HookConflict> {
        if self.pending.contains_key(&key) || self.active.contains_key(&key) {
            return Err(HookConflict);
        }
        self.pending.insert(key, pending);
        Ok(())
    }

    /// Promotes a pending hook into the active table.
    ///
    /// Activation intentionally reuses the original hook id and host path, but swaps the
    /// pending caller attribution into the active peer path used for data routing.
    pub fn activate_pending(&mut self, key: &HookKey) -> Option<()> {
        let pending = self.pending.remove(key)?;
        self.insert_active(
            key.clone(),
            ActiveHook {
                peer_path: pending.caller_src_path,
                procedure_id: pending.procedure_id,
                local_ended: pending.local_ended,
                peer_ended: false,
            },
        )
        .ok()?;
        Some(())
    }

    /// Inserts a live hook and its peer-path lookup entry.
    pub fn insert_active(&mut self, key: HookKey, active: ActiveHook) -> Result<(), HookConflict> {
        if self.pending.contains_key(&key)
            || self.active.contains_key(&key)
            || self
                .active_by_peer
                .get(&key.hook_id)
                .is_some_and(|peer_paths| peer_paths.contains_key(active.peer_path.as_slice()))
        {
            return Err(HookConflict);
        }
        self.active_by_peer
            .entry(key.hook_id)
            .or_default()
            .insert(active.peer_path.clone(), key.clone());
        self.active.insert(key, active);
        Ok(())
    }

    /// Removes a pending hook without affecting active state.
    pub fn remove_pending(&mut self, key: &HookKey) -> Option<PendingHook> {
        self.pending.remove(key)
    }

    /// Marks the local side finished before the hook becomes active.
    pub fn mark_pending_local_end(&mut self, key: &HookKey) {
        if let Some(pending) = self.pending.get_mut(key) {
            pending.local_ended = true;
        }
    }

    /// Removes an active hook and its secondary peer-path index entry.
    pub fn remove_active(&mut self, key: &HookKey) -> Option<ActiveHook> {
        let active = self.active.remove(key)?;
        if let Some(peer_paths) = self.active_by_peer.get_mut(&key.hook_id) {
            peer_paths.remove(active.peer_path.as_slice());
            if peer_paths.is_empty() {
                self.active_by_peer.remove(&key.hook_id);
            }
        }
        Some(active)
    }

    /// Returns the pending hook for `key`, if present.
    #[must_use]
    pub fn pending(&self, key: &HookKey) -> Option<&PendingHook> {
        self.pending.get(key)
    }

    /// Returns the active hook for `key`, if present.
    #[must_use]
    pub fn active(&self, key: &HookKey) -> Option<&ActiveHook> {
        self.active.get(key)
    }

    /// Returns the mutable active hook for `key`, if present.
    pub fn active_mut(&mut self, key: &HookKey) -> Option<&mut ActiveHook> {
        self.active.get_mut(key)
    }

    /// Resolves an active hook from either side of the conversation.
    ///
    /// The host side addresses hooks directly by `(return_path, hook_id)`. Peer-originated
    /// traffic only has `(hook_id, peer_path)`, so the secondary index maps that back to the
    /// canonical host-scoped key.
    #[must_use]
    pub fn resolve_active_key(
        &self,
        return_path: &[String],
        hook_id: u64,
        peer_path: &[String],
    ) -> Option<HookKey> {
        if let Some(key) = self
            .active_by_peer
            .get(&hook_id)
            .and_then(|peer_paths| peer_paths.get(peer_path))
        {
            return Some(key.clone());
        }

        let host_key = HookKey::new(return_path.to_vec(), hook_id);
        self.active.contains_key(&host_key).then_some(host_key)
    }

    /// Marks the local side finished and returns `true` once both sides are finished.
    pub fn mark_local_end(&mut self, key: &HookKey) -> bool {
        let Some(active) = self.active_mut(key) else {
            return false;
        };
        active.local_ended = true;
        active.peer_ended
    }

    /// Marks the peer side finished and returns `true` once both sides are finished.
    pub fn mark_peer_end(&mut self, key: &HookKey) -> bool {
        let Some(active) = self.active_mut(key) else {
            return false;
        };
        active.peer_ended = true;
        active.local_ended
    }

    /// Returns the number of active hooks.
    #[must_use]
    pub fn active_len(&self) -> usize {
        self.active.len()
    }

    /// Returns the number of pending hooks.
    #[must_use]
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }
}
