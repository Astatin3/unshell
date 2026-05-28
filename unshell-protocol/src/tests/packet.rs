use alloc::{string::ToString, vec, vec::Vec};

use crate::{DeserializeError, EndpointError, Packet, SerializeError};

// ── Helpers ───────────────────────────────────────────────────────────────

fn make_packet() -> Packet {
    Packet {
        hook_id: 42,
        end_hook: false,
        path: vec![1, 2, 3],
        procedure_id: "my.service.Method".to_string(),
        data: vec![0xDE, 0xAD, 0xBE, 0xEF],
    }
}

fn make_packet_flags(end_hook: bool) -> Packet {
    Packet {
        end_hook,
        ..make_packet()
    }
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
fn header_round_trip() {
    let packet = make_packet();
    let buf = packet.serialize().unwrap();
    let header = Packet::deserialize_header(&buf).unwrap();

    assert_eq!(header.hook_id, packet.hook_id);
    assert_eq!(header.end_hook, packet.end_hook);
    assert_eq!(header.path, packet.path);
}

// ── Flags ─────────────────────────────────────────────────────────────────

#[test]
fn flags_end_hook_false() {
    let packet = make_packet_flags(false);
    let buf = packet.serialize().unwrap();
    let header = Packet::deserialize_header(&buf).unwrap();
    assert!(!header.end_hook);
}

#[test]
fn flags_end_hook_true() {
    let packet = make_packet_flags(true);
    let buf = packet.serialize().unwrap();
    let header = Packet::deserialize_header(&buf).unwrap();
    assert!(header.end_hook);
}

// ── Empty fields ──────────────────────────────────────────────────────────

#[test]
fn empty_path() {
    let packet = Packet {
        path: vec![],
        ..make_packet()
    };
    let buf = packet.serialize().unwrap();
    let header = Packet::deserialize_header(&buf).unwrap();
    assert_eq!(header.path, &[] as &[u32]);
}

#[test]
fn empty_procedure_id() {
    let packet = Packet {
        procedure_id: "".to_string(),
        ..make_packet()
    };
    let buf = packet.serialize().unwrap();
    let result = Packet::deserialize(&buf).unwrap();
    assert_eq!(result.procedure_id, "");
}

#[test]
fn empty_data() {
    let packet = Packet {
        data: vec![],
        ..make_packet()
    };
    let buf = packet.serialize().unwrap();
    let result = Packet::deserialize(&buf).unwrap();
    assert_eq!(result.data, &[] as &[u8]);
}

#[test]
fn all_fields_empty() {
    let packet = Packet {
        hook_id: 0,
        end_hook: false,
        path: vec![],
        procedure_id: "".to_string(),
        data: vec![],
    };
    let buf = packet.serialize().unwrap();
    let result = Packet::deserialize(&buf).unwrap();
    assert_eq!(result.hook_id, 0);
    assert_eq!(result.path, Vec::<u32>::new());
    assert_eq!(result.procedure_id, "");
    assert_eq!(result.data, &[] as &[u8]);
}

// ── Zero-copy: borrows point into the original buffer ─────────────────────

#[test]
fn header_path_is_borrowed_from_buffer() {
    let buf = make_packet().serialize().unwrap();
    let header = Packet::deserialize_header(&buf).unwrap();

    let path_ptr = header.path.as_ptr() as *const u8;
    let buf_range = buf.as_ptr_range();
    assert!(
        buf_range.contains(&path_ptr),
        "path must be a subslice of the input buffer, not a new allocation"
    );
}

#[test]
fn body_remainder_is_borrowed_from_buffer() {
    let buf = make_packet().serialize().unwrap();
    let header = Packet::deserialize_header(&buf).unwrap();

    let remainder_ptr = header.body_remainder.as_ptr();
    let buf_range = buf.as_ptr_range();
    assert!(
        buf_range.contains(&remainder_ptr),
        "body_remainder must point into the input buffer"
    );
}

// ── Partial deserialization: body is untouched by header parse ────────────

#[test]
fn deserialize_header_does_not_read_body() {
    let buf = make_packet().serialize().unwrap();
    let header = Packet::deserialize_header(&buf).unwrap();

    // Re-parse body from the remainder to confirm it's intact.
    let body_buf = header.body_remainder;
    let body_len =
        u32::from_le_bytes([body_buf[0], body_buf[1], body_buf[2], body_buf[3]]) as usize;
    assert!(
        body_buf.len() >= 4 + body_len,
        "body_remainder must contain the full body"
    );
}

#[test]
fn can_forward_buffer_after_header_parse() {
    // Simulates a router: parse the header, then forward the raw buffer
    // without touching the body.
    let original = make_packet().serialize().unwrap();
    let header = Packet::deserialize_header(&original).unwrap();

    assert_eq!(header.path, &[1, 2, 3]);

    // "Forward" by deserializing the full original buffer downstream.
    let forwarded = Packet::deserialize(&original).unwrap();
    assert_eq!(forwarded.procedure_id, "my.service.Method");
    assert_eq!(forwarded.data, &[0xDE, 0xAD, 0xBE, 0xEF]);
}

// ── Truncation / corruption ───────────────────────────────────────────────

#[test]
fn truncated_in_fixed_prefix() {
    let buf = make_packet().serialize().unwrap();
    // Cut inside the fixed 8-byte prefix.
    assert_eq!(
        Packet::deserialize_header(&buf[..4]).unwrap_err(),
        DeserializeError::BufferTooShort
    );
}

#[test]
fn truncated_in_path() {
    let buf = make_packet().serialize().unwrap();
    // Cut to just past the fixed prefix, mid-path.
    assert_eq!(
        Packet::deserialize_header(&buf[..9]).unwrap_err(),
        DeserializeError::BufferTooShort
    );
}

#[test]
fn truncated_in_body() {
    let buf = make_packet().serialize().unwrap();
    // Remove last byte — well into the body.
    assert!(Packet::deserialize(&buf[..buf.len() - 1]).is_err());
}

#[test]
fn empty_buffer_rejected() {
    assert_eq!(
        Packet::deserialize_header(&[]).unwrap_err(),
        DeserializeError::BufferTooShort
    );
}

#[test]
fn invalid_utf8_in_procedure_id() {
    let mut buf = make_packet().serialize().unwrap();
    // Find where procedure_id starts: 8 + path_len*4 + 4 (body_len) + 4 (proc_id_len)
    let path_len = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]) as usize;
    let proc_id_offset = 8 + (path_len * 4) + 4 + 4;
    buf[proc_id_offset] = 0xFF;
    assert_eq!(
        Packet::deserialize(&buf).unwrap_err(),
        DeserializeError::InvalidUtf8
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
