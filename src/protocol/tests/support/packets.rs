use alloc::vec::Vec;

use crate::protocol::Packet;

/// Builds a test packet whose route is the only field varied by routing tests.
///
/// Keeping the payload stable makes each assertion about endpoint behavior rather
/// than packet construction, which is important because forged and malformed cases
/// should fail before any leaf-level procedure handling would matter.
pub(crate) fn echo_packet(path: Vec<u32>, hook_id: u16) -> Packet {
    echo_packet_with_end(path, hook_id, false)
}

/// Builds a test packet with an explicit hook-lifetime marker.
pub(crate) fn echo_packet_with_end(path: Vec<u32>, hook_id: u16, end_hook: bool) -> Packet {
    Packet {
        hook_id,
        end_hook,
        path,
        procedure_id: 1,
        data: "ABC123".as_bytes().to_vec(),
    }
}
