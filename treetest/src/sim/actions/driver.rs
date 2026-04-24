//! Simulator stepping helpers.

use crossbeam_channel::TryRecvError;
use unshell::protocol::decode_frame;
use unshell::protocol::tree::Endpoint;

use crate::model::NodeId;

use super::super::types::{SimError, Simulation};

impl Simulation {
    /// Processes one queued frame if available.
    pub fn step(&mut self) -> Result<bool, SimError> {
        for node_id in 0..self.nodes.len() {
            match self.nodes[node_id].rx.try_recv() {
                Ok(envelope) => {
                    // Record ingress before handing the frame to the protocol
                    // runtime so the trace shows the channel-level hop too.
                    self.record_trace(
                        NodeId(node_id),
                        format!("received frame via {:?}", envelope.ingress),
                    );
                    let outcome = self.nodes[node_id]
                        .endpoint
                        .receive(&envelope.ingress, envelope.frame)
                        .map_err(|error| SimError::Protocol(error.to_string()))?;
                    self.process_outcome(NodeId(node_id), outcome)?;
                    return Ok(true);
                }
                Err(TryRecvError::Disconnected) => {
                    return Err(SimError::Protocol("mailbox disconnected".to_owned()));
                }
                Err(TryRecvError::Empty) => {}
            }
        }
        Ok(false)
    }

    /// Runs frames until the network becomes idle.
    pub fn drain(&mut self) -> Result<usize, SimError> {
        // Count steps so callers can surface how much work one action caused.
        let mut steps = 0;
        while self.step()? {
            steps += 1;
        }
        Ok(steps)
    }

    /// Returns a compact description of a frame for debugging.
    pub fn describe_frame(frame: &[u8]) -> String {
        match decode_frame(frame) {
            Ok(parsed) => {
                let header = parsed.header();
                format!(
                    "{:?} {} -> {} hook {:?}",
                    header.packet_type,
                    crate::model::format_path(&header.src_path),
                    crate::model::format_path(&header.dst_path),
                    header.hook_id,
                )
            }
            Err(error) => format!("<invalid frame: {error}>"),
        }
    }
}
