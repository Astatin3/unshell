use alloc::{vec, vec::Vec};

use crate::protocol::{DeserializeError, EndpointError, Packet, SerializeError};

// ── Helpers ───────────────────────────────────────────────────────────────

fn make_packet() -> Packet {
    Packet {
        hook_id: 42,
        end_hook: false,
        path: vec![1, 2, 3],
        procedure_id: 0xAABB_CCDD,
        data: vec![0xDE, 0xAD, 0xBE, 0xEF],
    }
}

fn make_packet_flags(end_hook: bool) -> Packet {
    Packet {
        end_hook,
        ..make_packet()
    }
}

fn body_len_offset(buf: &[u8]) -> usize {
    let path_len = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]) as usize;
    8 + (path_len * 4)
}

fn procedure_id_offset(buf: &[u8]) -> usize {
    body_len_offset(buf) + 4
}

// ── Round-trip ────────────────────────────────────────────────────────────

#[test]
fn full_round_trip() {
    let packet = make_packet();
    let buf = packet.serialize().unwrap();
    let result = Packet::deserialize(&buf).unwrap();

    assert_eq!(result.hook_id, packet.hook_id);
    assert_eq!(result.end_hook, packet.end_hook);
    assert_eq!(result.path, packet.path);
    assert_eq!(result.procedure_id, packet.procedure_id);
    assert_eq!(result.data, packet.data);
}

#[test]
fn procedure_id_is_fixed_width_u32() {
    let packet = make_packet();
    let buf = packet.serialize().unwrap();
    let proc_offset = procedure_id_offset(&buf);

    assert_eq!(
        &buf[proc_offset..proc_offset + 4],
        &packet.procedure_id.to_le_bytes()
    );
    assert_eq!(&buf[proc_offset + 4..], packet.data.as_slice());
}

// ── Flags ─────────────────────────────────────────────────────────────────

#[test]
fn flags_end_hook_false() {
    let packet = make_packet_flags(false);
    let result = Packet::deserialize(&packet.serialize().unwrap()).unwrap();
    assert!(!result.end_hook);
}

#[test]
fn flags_end_hook_true() {
    let packet = make_packet_flags(true);
    let result = Packet::deserialize(&packet.serialize().unwrap()).unwrap();
    assert!(result.end_hook);
}

// ── Empty fields ──────────────────────────────────────────────────────────

#[test]
fn empty_path() {
    let packet = Packet {
        path: vec![],
        ..make_packet()
    };
    let result = Packet::deserialize(&packet.serialize().unwrap()).unwrap();
    assert_eq!(result.path, &[] as &[u32]);
}

#[test]
fn zero_procedure_id() {
    let packet = Packet {
        procedure_id: 0,
        ..make_packet()
    };
    let result = Packet::deserialize(&packet.serialize().unwrap()).unwrap();
    assert_eq!(result.procedure_id, 0);
}

#[test]
fn empty_data() {
    let packet = Packet {
        data: vec![],
        ..make_packet()
    };
    let result = Packet::deserialize(&packet.serialize().unwrap()).unwrap();
    assert_eq!(result.data, &[] as &[u8]);
}

#[test]
fn all_fields_empty() {
    let packet = Packet {
        hook_id: 0,
        end_hook: false,
        path: vec![],
        procedure_id: 0,
        data: vec![],
    };
    let result = Packet::deserialize(&packet.serialize().unwrap()).unwrap();
    assert_eq!(result.hook_id, 0);
    assert_eq!(result.path, Vec::<u32>::new());
    assert_eq!(result.procedure_id, 0);
    assert_eq!(result.data, &[] as &[u8]);
}

// ── Truncation / corruption ───────────────────────────────────────────────

#[test]
fn truncated_in_fixed_prefix() {
    let buf = make_packet().serialize().unwrap();
    // Cut inside the fixed 8-byte prefix.
    assert_eq!(
        Packet::deserialize(&buf[..4]).unwrap_err(),
        DeserializeError::BufferTooShort
    );
}

#[test]
fn truncated_in_path() {
    let buf = make_packet().serialize().unwrap();
    // Cut to just past the fixed prefix, mid-path.
    assert_eq!(
        Packet::deserialize(&buf[..9]).unwrap_err(),
        DeserializeError::BufferTooShort
    );
}

#[test]
fn truncated_before_body_len() {
    let buf = make_packet().serialize().unwrap();
    let body_len_offset = body_len_offset(&buf);

    assert_eq!(
        Packet::deserialize(&buf[..body_len_offset + 2]).unwrap_err(),
        DeserializeError::BufferTooShort
    );
}

#[test]
fn truncated_in_body() {
    let buf = make_packet().serialize().unwrap();
    // Remove last byte — well into the body.
    assert_eq!(
        Packet::deserialize(&buf[..buf.len() - 1]).unwrap_err(),
        DeserializeError::BodyLengthMismatch
    );
}

#[test]
fn empty_buffer_rejected() {
    assert_eq!(
        Packet::deserialize(&[]).unwrap_err(),
        DeserializeError::BufferTooShort
    );
}

#[test]
fn body_length_mismatch_is_rejected() {
    let mut buf = make_packet().serialize().unwrap();
    let body_len_offset = body_len_offset(&buf);
    let inflated_body_len = 999u32;
    buf[body_len_offset..body_len_offset + 4].copy_from_slice(&inflated_body_len.to_le_bytes());

    assert_eq!(
        Packet::deserialize(&buf).unwrap_err(),
        DeserializeError::BodyLengthMismatch
    );
}

#[test]
fn body_too_short_for_procedure_id_is_rejected() {
    let mut buf = make_packet().serialize().unwrap();
    let body_len_offset = body_len_offset(&buf);
    let short_body_len = 3u32;
    buf[body_len_offset..body_len_offset + 4].copy_from_slice(&short_body_len.to_le_bytes());

    assert_eq!(
        Packet::deserialize(&buf).unwrap_err(),
        DeserializeError::BufferTooShort
    );
}

#[test]
fn serialize_error_wraps_into_endpoint_error() {
    let error: EndpointError = SerializeError::BodyTooLarge.into();

    assert_eq!(
        error,
        EndpointError::PacketSerialize {
            source: SerializeError::BodyTooLarge,
        }
    );
}

#[test]
fn deserialize_error_wraps_into_endpoint_error() {
    let error: EndpointError = DeserializeError::BufferTooShort.into();

    assert_eq!(
        error,
        EndpointError::PacketDeserialize {
            source: DeserializeError::BufferTooShort,
        }
    );
}
