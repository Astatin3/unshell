use alloc::vec::Vec;

use crate::protocol::{Endpoint, EndpointError, Packet};

use super::HookID;

impl Endpoint {
    /// Returns the destination path for packets sent back over `hook_id`.
    ///
    /// Hooks record the adjacent peer that paved the return channel. This helper turns
    /// that peer into the packet path required by the current router: parent peers map
    /// to the parent path, and child peers map to the direct child path. Session logic
    /// should not store this path itself.
    pub(crate) fn hook_path(&self, hook_id: HookID) -> Result<Vec<u32>, EndpointError> {
        let peer = self
            .hook_peer(hook_id)
            .ok_or(EndpointError::UnknownHook { hook_id })?;

        if self.path.is_empty() {
            return Err(EndpointError::EndpointPathUnset);
        }

        if self.path.len() > 1 && self.path[self.path.len() - 2] == peer {
            Ok(self.path[..self.path.len() - 1].to_vec())
        } else {
            let mut path = self.path.clone();
            path.push(peer);
            Ok(path)
        }
    }

    /// Routes raw response data over an existing hook immediately.
    ///
    /// This is the compact session-output path: it avoids an intermediate context and
    /// retry queue. If a final packet cannot route, the local hook is still removed so
    /// an implant does not retain dead hook state forever.
    pub fn send_hook_raw(
        &mut self,
        hook_id: HookID,
        procedure_id: u32,
        data: Vec<u8>,
        end_hook: bool,
    ) -> Result<(), EndpointError> {
        let path = self.hook_path(hook_id)?;
        let packet = Packet {
            hook_id,
            end_hook,
            path,
            procedure_id,
            data,
        };

        let result = self.add_outbound(packet);

        if result.is_err() && end_hook {
            self.close_hook(hook_id);
        }

        result
    }

    /// Routes a one-byte-opcode response frame over an existing hook immediately.
    pub fn send_hook_frame(
        &mut self,
        hook_id: HookID,
        procedure_id: u32,
        opcode: u8,
        payload: &[u8],
        end_hook: bool,
    ) -> Result<(), EndpointError> {
        let mut data = Vec::with_capacity(payload.len() + 1);
        data.push(opcode);
        data.extend_from_slice(payload);

        self.send_hook_raw(hook_id, procedure_id, data, end_hook)
    }
}
