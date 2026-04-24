//! Stateless protocol validation.

use core::fmt;

use crate::protocol::{
    CallMessage, PacketHeader, PacketType, introspection::INTROSPECTION_PROCEDURE_ID,
};

/// Validation failures for protocol structures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// Header invariants were violated.
    HeaderInvariant(&'static str),
    /// The canonical procedure identifier was invalid.
    ProcedureId(&'static str),
    /// Call-specific invariants were violated.
    CallInvariant(&'static str),
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HeaderInvariant(message) => write!(f, "invalid header: {message}"),
            Self::ProcedureId(message) => write!(f, "invalid procedure id: {message}"),
            Self::CallInvariant(message) => write!(f, "invalid call: {message}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ValidationError {}

/// Validates packet header invariants from the protocol.
///
/// # Errors
///
/// Returns [`ValidationError`] when the header shape does not match the packet type.
pub fn validate_header(header: &PacketHeader) -> Result<(), ValidationError> {
    match header.packet_type {
        PacketType::Call => {
            if header.hook_id.is_some() {
                return Err(ValidationError::HeaderInvariant(
                    "Call packets must not carry hook_id",
                ));
            }
        }
        PacketType::Data | PacketType::Fault => {
            if header.dst_leaf.is_some() {
                return Err(ValidationError::HeaderInvariant(
                    "Data and Fault packets must not carry dst_leaf",
                ));
            }
            if header.hook_id.is_none() {
                return Err(ValidationError::HeaderInvariant(
                    "Data and Fault packets must carry hook_id",
                ));
            }
        }
    }

    Ok(())
}

/// Validates the canonical dotted `procedure_id` shape.
///
/// # Errors
///
/// Returns [`ValidationError`] when the procedure id does not match the required format.
pub fn validate_procedure_id(procedure_id: &str) -> Result<(), ValidationError> {
    if procedure_id == INTROSPECTION_PROCEDURE_ID {
        return Ok(());
    }

    let mut segments = procedure_id.split('.');
    let mut collected = [""; 5];
    for (index, slot) in collected.iter_mut().enumerate() {
        let Some(segment) = segments.next() else {
            return Err(ValidationError::ProcedureId(
                "must contain exactly 5 segments",
            ));
        };
        if segment.is_empty() {
            return Err(ValidationError::ProcedureId("segments must be non-empty"));
        }
        *slot = segment;
        if index != 2 && !segment.chars().all(is_portable_procedure_char) {
            return Err(ValidationError::ProcedureId(
                "segments should use lowercase ASCII, digits, and underscores",
            ));
        }
    }

    if segments.next().is_some() {
        return Err(ValidationError::ProcedureId(
            "must contain exactly 5 segments",
        ));
    }

    let version = collected[2];
    let Some(suffix) = version.strip_prefix('v') else {
        return Err(ValidationError::ProcedureId(
            "third segment must be a version like v1",
        ));
    };

    if suffix.is_empty() || suffix.starts_with('0') || !suffix.chars().all(|ch| ch.is_ascii_digit())
    {
        return Err(ValidationError::ProcedureId(
            "version segment must be v followed by a positive decimal integer",
        ));
    }

    Ok(())
}

/// Validates call-specific invariants that depend on both header and payload.
///
/// # Errors
///
/// Returns [`ValidationError`] when the call payload conflicts with the header.
pub fn validate_call(header: &PacketHeader, call: &CallMessage) -> Result<(), ValidationError> {
    validate_procedure_id(&call.procedure_id)?;

    if let Some(hook) = &call.response_hook
        && hook.return_path != header.src_path
    {
        return Err(ValidationError::CallInvariant(
            "response_hook.return_path must equal header.src_path",
        ));
    }

    if call.procedure_id == INTROSPECTION_PROCEDURE_ID && call.response_hook.is_none() {
        return Err(ValidationError::CallInvariant(
            "introspection requires a response hook",
        ));
    }

    Ok(())
}

fn is_portable_procedure_char(ch: char) -> bool {
    ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_'
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{HookTarget, PacketType};
    use alloc::{string::String, vec};

    #[test]
    fn rejects_invalid_data_header() {
        let header = PacketHeader {
            packet_type: PacketType::Data,
            src_path: Vec::new(),
            dst_path: Vec::new(),
            dst_leaf: Some(String::from("leaf")),
            hook_id: None,
        };
        assert!(validate_header(&header).is_err());
    }

    #[test]
    fn validates_procedure_id_shape() {
        assert!(validate_procedure_id("org.product.v1.demo.echo").is_ok());
        assert!(validate_procedure_id("org.product.v01.demo.echo").is_err());
        assert!(validate_procedure_id("Org.product.v1.demo.echo").is_err());
    }

    #[test]
    fn validates_response_hook_return_path() {
        let header = PacketHeader {
            packet_type: PacketType::Call,
            src_path: vec![String::from("src")],
            dst_path: vec![String::from("dst")],
            dst_leaf: None,
            hook_id: None,
        };
        let call = CallMessage {
            procedure_id: String::from("org.product.v1.demo.echo"),
            data: Vec::new(),
            response_hook: Some(HookTarget {
                hook_id: 1,
                return_path: vec![String::from("other")],
            }),
        };
        assert!(validate_call(&header, &call).is_err());
    }
}
