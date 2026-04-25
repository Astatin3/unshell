use alloc::{borrow::ToOwned, string::String, vec, vec::Vec};

use crate::protocol::{
    CallMessage, FaultMessage, FrameError, HookTarget, PacketHeader, PacketType, ProtocolFault,
    SECTION_ALIGN, ValidationError, decode_frame, encode_packet, validate_call, validate_header,
    validate_procedure_id,
};

fn path(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_owned()).collect()
}

#[test]
fn packet_framing_roundtrip_preserves_header_and_payload() {
    let header = PacketHeader {
        packet_type: PacketType::Call,
        src_path: path(&["root", "caller"]),
        dst_path: path(&["root", "callee"]),
        dst_leaf: Some("service".to_owned()),
        hook_id: None,
    };
    let call = CallMessage {
        procedure_id: "example.service.v1.invoke".to_owned(),
        data: vec![1, 2, 3, 4],
        response_hook: Some(HookTarget {
            hook_id: 7,
            return_path: path(&["root", "caller"]),
        }),
    };

    let frame = encode_packet(&header, &call).expect("frame should encode");
    assert_eq!(frame.as_ptr() as usize % SECTION_ALIGN, 0);
    let parsed = decode_frame(&frame).expect("frame should decode");

    assert_eq!(parsed.header(), &header);
    assert_eq!(parsed.packet_type(), PacketType::Call);
    assert_eq!(
        parsed.deserialize_call().expect("call should deserialize"),
        call
    );
}

#[test]
fn header_and_call_validation_reject_invalid_combinations() {
    let invalid_header = PacketHeader {
        packet_type: PacketType::Data,
        src_path: path(&["peer"]),
        dst_path: path(&["host"]),
        dst_leaf: Some("service".to_owned()),
        hook_id: None,
    };
    assert_eq!(
        validate_header(&invalid_header),
        Err(ValidationError::HeaderInvariant(
            "Data and Fault packets must not carry dst_leaf"
        ))
    );

    let header = PacketHeader {
        packet_type: PacketType::Call,
        src_path: path(&["caller"]),
        dst_path: path(&["callee"]),
        dst_leaf: Some("service".to_owned()),
        hook_id: None,
    };
    let invalid_call = CallMessage {
        procedure_id: "example.service.v1.invoke".to_owned(),
        data: Vec::new(),
        response_hook: Some(HookTarget {
            hook_id: 5,
            return_path: path(&["elsewhere"]),
        }),
    };
    assert_eq!(
        validate_call(&header, &invalid_call),
        Err(ValidationError::CallInvariant(
            "response_hook.return_path must equal header.src_path"
        ))
    );
}

#[test]
fn procedure_validation_accepts_introspection_and_non_empty_opaque_ids() {
    assert_eq!(validate_procedure_id(""), Ok(()));
    assert_eq!(validate_procedure_id("example.service.v01.invoke"), Ok(()));
    assert_eq!(validate_procedure_id("contains spaces"), Ok(()));
}

#[test]
fn truncated_frames_are_rejected() {
    let header = PacketHeader {
        packet_type: PacketType::Fault,
        src_path: path(&["src"]),
        dst_path: path(&["dst"]),
        dst_leaf: None,
        hook_id: Some(9),
    };
    let message = FaultMessage {
        fault: ProtocolFault::INTERNAL_ERROR,
    };

    let frame = encode_packet(&header, &message).expect("frame should encode");
    let truncated = &frame[..frame.len() - 1];

    assert!(matches!(
        decode_frame(truncated),
        Err(FrameError::Truncated)
    ));
}
