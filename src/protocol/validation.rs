//! Stateless protocol validation.

use crate::protocol::{
    CallMessage, PacketHeader, PacketType, introspection::INTROSPECTION_PROCEDURE_ID,
};
use core::fmt;

/// Validation failures for protocol structures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// Header invariants were violated.
    HeaderInvariant(&'static str),
    /// The canonical procedure identifier was invalid.
    ProcedureId(&'static str),
    /// Call-specific invariants were violated.
    CallInvariant(&'static str),
    /// The hook identifier is already in use.
    InvalidHookId,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HeaderInvariant(message) => write!(f, "invalid header: {message}"),
            Self::ProcedureId(message) => write!(f, "invalid procedure id: {message}"),
            Self::CallInvariant(message) => write!(f, "invalid call: {message}"),
            Self::InvalidHookId => write!(f, "invalid hook identifier"),
        }
    }
}

impl core::error::Error for ValidationError {}

/// Validates packet header invariants from the protocol.
///
/// This checks only the header fields themselves. Payload-dependent rules belong
/// in helpers such as [`validate_call`].
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
pub fn validate_procedure_id(procedure_id: &str) -> Result<(), ValidationError> {
    if procedure_id == INTROSPECTION_PROCEDURE_ID {
        return Ok(());
    }

    if procedure_id.is_empty() {
        return Err(ValidationError::ProcedureId(
            "procedure identifier cannot be empty except for introspection",
        ));
    }

    if !procedure_id
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '.')
    {
        return Err(ValidationError::ProcedureId(
            "procedure identifier should use alphanumeric characters, dots, and underscores",
        ));
    }

    Ok(())
}

/// Validates call-specific invariants that depend on both header and payload.
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
