//! Stateless protocol validation.

use crate::protocol::{
    CallMessage, PacketHeader, PacketType, introspection::INTROSPECTION_PROCEDURE_ID,
};
use core::fmt;

/// Validation failures for protocol structures.
///
/// These errors exist so callers can reject malformed outbound packets early, before they are
/// encoded or sent across the tree.
///
/// # Example
/// ```rust
/// use unshell::protocol::{PacketHeader, PacketType, ValidationError, validate_header};
/// let invalid = PacketHeader {
///     packet_type: PacketType::Data,
///     src_path: vec!["peer".into()],
///     dst_path: vec!["host".into()],
///     dst_leaf: Some("service".into()),
///     hook_id: None,
/// };
/// assert!(matches!(validate_header(&invalid), Err(ValidationError::HeaderInvariant(_))));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// One header field combination is invalid for the chosen packet type.
    HeaderInvariant(&'static str),
    /// The procedure identifier violates the protocol's minimal reserved-id rules.
    ProcedureId(&'static str),
    /// The call payload contradicts the surrounding packet header.
    CallInvariant(&'static str),
    /// A hook lifecycle transition would break protocol state invariants.
    HookInvariant(&'static str),
    /// One endpoint-topology update would break local tree invariants.
    TopologyInvariant(&'static str),
    /// A hook id collided with existing endpoint-local state.
    InvalidHookId,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HeaderInvariant(message) => write!(f, "invalid header: {message}"),
            Self::ProcedureId(message) => write!(f, "invalid procedure id: {message}"),
            Self::CallInvariant(message) => write!(f, "invalid call: {message}"),
            Self::HookInvariant(message) => write!(f, "invalid hook state: {message}"),
            Self::TopologyInvariant(message) => write!(f, "invalid topology: {message}"),
            Self::InvalidHookId => f.write_str("invalid hook identifier"),
        }
    }
}

impl core::error::Error for ValidationError {}

/// Validates stateless packet-header invariants.
///
/// This checks wire-shape rules only. It does not verify route existence, leaf existence,
/// hook ownership, or whether the destination actually supports the requested procedure.
///
/// # Example
/// ```rust
/// use unshell::protocol::{PacketHeader, PacketType, validate_header};
/// let header = PacketHeader {
///     packet_type: PacketType::Call,
///     src_path: vec!["root".into()],
///     dst_path: vec!["worker".into()],
///     dst_leaf: Some("service".into()),
///     hook_id: None,
/// };
/// validate_header(&header)?;
/// # Ok::<(), unshell::protocol::ValidationError>(())
/// ```
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

/// Validates the protocol-level `procedure_id` invariant.
///
/// This is intentionally permissive. The protocol reserves only the empty string for
/// introspection; every other non-empty identifier is treated as opaque application data.
///
/// # Example
/// ```rust
/// use unshell::protocol::{INTROSPECTION_PROCEDURE_ID, validate_procedure_id};
/// validate_procedure_id(INTROSPECTION_PROCEDURE_ID)?;
/// validate_procedure_id("example.service.v1.invoke")?;
/// # Ok::<(), unshell::protocol::ValidationError>(())
/// ```
pub fn validate_procedure_id(procedure_id: &str) -> Result<(), ValidationError> {
    if procedure_id == INTROSPECTION_PROCEDURE_ID {
        return Ok(());
    }
    if procedure_id.is_empty() {
        return Err(ValidationError::ProcedureId(
            "procedure identifier cannot be empty except for introspection",
        ));
    }
    Ok(())
}

/// Validates call-specific invariants that depend on both header and payload.
///
/// This complements [`validate_header`]. It does not verify destination reachability or leaf
/// support, only consistency between the opening `Call` header and payload.
///
/// # Example
/// ```rust
/// use unshell::protocol::{CallMessage, HookTarget, PacketHeader, PacketType, validate_call};
/// let header = PacketHeader {
///     packet_type: PacketType::Call,
///     src_path: vec!["root".into()],
///     dst_path: vec!["worker".into()],
///     dst_leaf: Some("service".into()),
///     hook_id: None,
/// };
/// let call = CallMessage {
///     procedure_id: "example.service.v1.invoke".into(),
///     data: vec![],
///     response_hook: Some(HookTarget {
///         hook_id: 7,
///         return_path: vec!["root".into()],
///     }),
/// };
/// validate_call(&header, &call)?;
/// # Ok::<(), unshell::protocol::ValidationError>(())
/// ```
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
        // Introspection is defined as a request/response exchange, never a fire-and-forget call.
        return Err(ValidationError::CallInvariant(
            "introspection requires a response hook",
        ));
    }

    Ok(())
}
