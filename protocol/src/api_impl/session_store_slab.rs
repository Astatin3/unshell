use crate::api::session::{SessionData, SessionStore};

/// Slab/arena implementation: a Vec of slots plus a free-list, so closed
/// sessions' slots are recycled instead of the Vec only ever growing.
/// O(1) insert/get/remove, no hashing, good cache locality for the
/// "iterate all live sessions" case (heartbeats, timeouts).
pub struct SlabSessionStore {
    slots: Vec<Slot>,
    free_head: Option<u32>,
}

enum Slot {
    Occupied {
        generation: u32,
        data: SessionData,
    },
    Free {
        generation: u32,
        next_free: Option<u32>,
    },
}

/// Handle to a session, valid only for the Node that issued it.
///
/// Generational index instead of a UUID/HashMap key:
///   - Copy, 8 bytes, no allocation, no hashing to look up.
///   - `generation` invalidates stale handles after a slot is reused,
///     which is the failure mode a raw `Vec` index or a bare `u32`
///     session counter can't catch on its own.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct SlabId {
    index: u32,
    generation: u32,
}

impl SessionStore for SlabSessionStore {
    type Key = SlabId;

    fn insert(&mut self, data: SessionData) -> SlabId {
        if let Some(idx) = self.free_head {
            let slot = &mut self.slots[idx as usize];
            let Slot::Free {
                generation,
                next_free,
            } = *slot
            else {
                unreachable!("free_head must point at a free slot")
            };
            self.free_head = next_free;
            *slot = Slot::Occupied { generation, data };
            SlabId {
                index: idx,
                generation,
            }
        } else {
            let generation = 0;
            let idx = self.slots.len() as u32;
            self.slots.push(Slot::Occupied { generation, data });
            SlabId {
                index: idx,
                generation,
            }
        }
    }

    fn get(&self, id: &SlabId) -> Option<&SessionData> {
        match self.slots.get(id.index as usize) {
            Some(Slot::Occupied { generation, data }) if *generation == id.generation => Some(data),
            _ => None,
        }
    }

    fn get_mut(&mut self, id: &SlabId) -> Option<&mut SessionData> {
        match self.slots.get_mut(id.index as usize) {
            Some(Slot::Occupied { generation, data }) if *generation == id.generation => Some(data),
            _ => None,
        }
    }

    fn remove(&mut self, id: &SlabId) -> Option<SessionData> {
        let slot = self.slots.get_mut(id.index as usize)?;
        match *slot {
            Slot::Occupied { generation, .. } if generation == id.generation => {
                let Slot::Occupied { generation, data } = std::mem::replace(
                    slot,
                    Slot::Free {
                        generation: 0,
                        next_free: None,
                    },
                ) else {
                    unreachable!()
                };
                *slot = Slot::Free {
                    generation: generation.wrapping_add(1),
                    next_free: self.free_head,
                };
                self.free_head = Some(id.index);
                Some(data)
            }
            _ => None,
        }
    }
}
