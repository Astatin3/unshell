use std::collections::HashSet;

use crate::{HookID, NodeID, ProcedureID, packet::PacketHeader};

pub struct Router {
    /// This node's ID
    pub id: NodeID,

    /// The node IDs of all parents in decending order.
    /// The 0th index is the root node, and the last is
    /// the parent.
    ///
    /// This may be blank if this is the root node,
    /// or a parent hasn't been established yet.
    pub this_path: Vec<NodeID>,

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

    pub fn recv<F, G, H>(
        &mut self,
        packet: PacketHeader,

        // Called when a packet must be immediately written
        // to some stream
        callback_relay: F,

        // Called when a packet should be processed
        callback_recv: G,

        // Called when a packet is malformed,
        // it's packet data should be cleared
        callback_malformed: H,
    ) where
        F: FnMut(PacketHeader),
        G: FnMut(PacketHeader),
        H: FnMut(),
    {
        if packet.is_downwards() {
            self.recv_downwards(packet, callback_relay, callback_recv, callback_malformed);
        } else {
            self.recv_upwards(packet, callback_relay, callback_recv, callback_malformed);
        }
    }

    fn recv_downwards<F, G, H>(
        &mut self,
        mut packet: PacketHeader,
        mut callback_relay: F,
        mut callback_recv: G,
        mut callback_malformed: H,
    ) where
        F: FnMut(PacketHeader),
        G: FnMut(PacketHeader),
        H: FnMut(),
    {
        let this_depth = self.this_path.len();
        let path_len = packet.path.len();
        let depth_number = packet.depth_number as usize;

        // This can never happen
        if depth_number > this_depth || path_len > this_depth || depth_number > path_len {
            return callback_malformed();
        }

        // Get source node
        // depth_number = (src_path_len + hops) - src_index
        // depth_number = this_depth - src_index
        // src_index = this_depth - depth_number
        let upper_node = match packet.path.get(this_depth - depth_number) {
            Some(node) => *node,
            None => return callback_malformed(),
        };

        // Get destination node
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

        let last_node = this_depth == path_len - 1;
        let correct_last_node = self.id == lower_node;

        if last_node && correct_last_node {
            return callback_recv(packet);
        } else if !last_node && !correct_last_node {
            // If this overflows it will make the next router just drop the packet
            packet.depth_number = packet.depth_number.saturating_add(1);

            return callback_relay(packet);
        }

        callback_malformed();
    }

    fn recv_upwards<F, G, H>(
        &mut self,
        mut packet: PacketHeader,
        mut callback_relay: F,
        mut callback_recv: G,
        mut callback_malformed: H,
    ) where
        F: FnMut(PacketHeader),
        G: FnMut(PacketHeader),
        H: FnMut(),
    {
        let this_depth = self.this_path.len();
        let depth_number = packet.depth_number as usize;

        if depth_number > this_depth {
            return callback_malformed();
        }

        let lower_node = match packet.path.first() {
            Some(node) => *node,
            None => return callback_malformed(),
        };

        // If the packet is sent to the root
        if depth_number == 0 {
            let hook = (self.id, lower_node, packet.hook_id, packet.procedure_id);

            if !self.hook_exists(&hook) {
                return callback_malformed();
            }

            if packet.is_close() {
                self.check_remove_hook(&hook);
            }

            return callback_recv(packet);
        } else {
            // Get upper node position
            let upper_node = match self.this_path.get(this_depth - depth_number) {
                Some(dst_node) => *dst_node,
                None => return callback_malformed(),
            };

            let hook = (upper_node, lower_node, packet.hook_id, packet.procedure_id);

            if !self.hook_exists(&hook) {
                return callback_malformed();
            }

            if packet.is_close() {
                self.check_remove_hook(&hook);
            }

            packet.depth_number -= 1;
            packet.path.push(self.id);

            return callback_relay(packet);
        }
    }
}
