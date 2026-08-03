use std::collections::HashSet;

use crate::{HookID, NodeID, ProcedureID, packet::PacketHeader};

pub struct Router {
    /// This node's ID
    id: NodeID,

    /// The node IDs of all parents in decending order.
    /// The 0th index is the root node, and the last is
    /// the parent.
    ///
    /// This may be blank if this is the root node,
    /// or a parent hasn't been established yet.
    this_path: Vec<NodeID>,

    /// Map of registered hooks
    ///
    /// Key
    /// 0: upper node ID
    /// 1: lower Node ID
    /// 2: Hook id
    /// 3: Procedure id
    hooks: HashSet<(NodeID, NodeID, HookID, ProcedureID)>,
}

impl Router {
    fn check_create_hook(&mut self, hook: (NodeID, NodeID, HookID, ProcedureID)) -> bool {
        self.hooks.insert(hook)
    }

    fn check_remove_hook(&mut self, hook: &(NodeID, NodeID, HookID, ProcedureID)) -> bool {
        self.hooks.remove(hook)
    }

    pub fn hook_exists(&self, hook: &(NodeID, NodeID, HookID, ProcedureID)) -> bool {
        self.hooks.contains(hook)
    }

    pub fn recv_downwards<F, G, H>(
        &mut self,
        mut packet: PacketHeader,

        // Called when a packet must be immediately written
        // to some stream
        mut callback_relay: F,

        // Called when a packet should be processed
        mut callback_recv: G,

        // Called when a packet is malformed,
        // it's packet data should be cleared
        mut callback_malformed: H,
    ) where
        F: FnMut(PacketHeader),
        G: FnMut(PacketHeader),
        H: FnMut(),
    {
        let this_depth = self.this_path.len();
        let path_len = packet.path.len();
        let depth_number = packet.depth_number as usize;

        // The depth number must always match the depth of this endpoint
        if depth_number != this_depth {
            return callback_malformed();
        }

        let upper_node = match packet.path.get(packet.src_index as usize) {
            Some(node) => *node,

            None => return callback_malformed(),
        };

        let lower_node = match packet.path.last() {
            Some(node) => *node,
            None => return callback_malformed(), // malformed
        };

        let hook = (upper_node, lower_node, packet.hook_id, packet.procedure_id);

        if !packet.is_close() {
            self.check_create_hook(hook);
        } else {
            self.check_remove_hook(&hook);
        }

        let last_node = depth_number + 1 == path_len;
        let correct_last_node = self.id == lower_node;

        if last_node && correct_last_node {
            callback_recv(packet);
        } else if !last_node && !correct_last_node {
            // If this overflows it will make the next router just drop the packet
            packet.depth_number = packet.depth_number.saturating_add(1);

            callback_relay(packet);
        }

        callback_malformed();
    }

    pub fn recv_upwards<F, G, H>(
        &mut self,
        mut packet: PacketHeader,

        // Called when a packet must be immediately written
        // to some stream
        mut callback_relay: F,

        // Called when a packet should be processed
        mut callback_recv: G,

        // Called when a packet is malformed,
        // it's packet data should be cleared
        mut callback_malformed: H,
    ) where
        F: FnMut(PacketHeader),
        G: FnMut(PacketHeader),
        H: FnMut(),
    {
        let this_depth = self.this_path.len();
        let depth_number = packet.depth_number as usize;

        if depth_number != this_depth {
            return callback_malformed();
        }

        let src_node = match packet.path.get(depth_number) {
            Some(node) => *node,
            None => return callback_malformed(),
        };

        match depth_number
            .checked_sub(1)
            .and_then(|i| self.this_path.get(i))
        {
            Some(dst_node) => {
                let dst_node = *dst_node;
                let hook = (src_node, dst_node, packet.hook_id, packet.procedure_id);

                if !self.hook_exists(&hook) {
                    return callback_malformed();
                }

                if packet.is_close() {
                    self.check_remove_hook(&hook);
                }

                packet.depth_number -= 1;
                packet.path.push(self.id);

                callback_relay(packet);
            }

            // This packet is sent to the root
            None => {
                let hook = (src_node, self.id, packet.hook_id, packet.procedure_id);

                if !self.hook_exists(&hook) {
                    return callback_malformed();
                }

                if packet.is_close() {
                    self.check_remove_hook(&hook);
                }

                callback_recv(packet)
            }
        }
    }
}
