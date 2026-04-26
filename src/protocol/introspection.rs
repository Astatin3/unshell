//! Required introspection payloads for discovery.
//!
//! These types define the reserved discovery subsystem of the protocol. Endpoints use the
//! reserved empty-string procedure id to request either endpoint-wide discovery or one leaf's
//! exact procedure inventory.
//!
//! # Example
//! ```rust
//! use unshell::protocol::{EndpointIntrospection, INTROSPECTION_PROCEDURE_ID};
//! let payload = EndpointIntrospection {
//!     sub_endpoints: vec!["worker".into()],
//!     leaves: vec![],
//! };
//! assert_eq!(INTROSPECTION_PROCEDURE_ID, "");
//! assert_eq!(payload.sub_endpoints[0], "worker");
//! ```

use alloc::{string::String, vec::Vec};
use rkyv::{Archive, Deserialize, Serialize};

/// Reserved procedure id for protocol introspection.
///
/// The protocol uses the empty string here so discovery traffic stays outside the normal
/// application procedure namespace. [`crate::protocol::validate_procedure_id`] reserves that
/// value exclusively for introspection.
///
/// # Example
/// ```rust
/// use unshell::protocol::INTROSPECTION_PROCEDURE_ID;
/// assert!(INTROSPECTION_PROCEDURE_ID.is_empty());
/// ```
pub const INTROSPECTION_PROCEDURE_ID: &str = "";

/// Endpoint-wide introspection payload.
///
/// This is returned when discovery targets an endpoint path without selecting one specific leaf.
/// It exists so clients can enumerate direct child endpoints and the leaves hosted locally.
///
/// # Example
/// ```rust
/// use unshell::protocol::EndpointIntrospection;
/// let payload = EndpointIntrospection {
///     sub_endpoints: vec!["worker".into()],
///     leaves: vec![],
/// };
/// assert_eq!(payload.sub_endpoints.len(), 1);
/// ```
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct EndpointIntrospection {
    /// Direct child endpoint segment names hosted immediately below this endpoint.
    pub sub_endpoints: Vec<String>,
    /// Leaf summaries hosted directly at this endpoint.
    pub leaves: Vec<LeafIntrospectionSummary>,
}

/// Shared per-leaf discovery record.
///
/// This compact shape exists so endpoint-wide discovery can advertise each hosted leaf without
/// sending the full endpoint envelope again.
///
/// # Example
/// ```rust
/// use unshell::protocol::LeafIntrospectionSummary;
/// let summary = LeafIntrospectionSummary {
///     leaf_name: "org.example.v1.echo".into(),
///     procedures: vec!["org.example.v1.echo.invoke".into()],
/// };
/// assert_eq!(summary.procedures.len(), 1);
/// ```
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct LeafIntrospectionSummary {
    /// Canonical dotted leaf identifier.
    pub leaf_name: String,
    /// Exhaustive canonical procedure ids currently exposed by the leaf.
    pub procedures: Vec<String>,
}

/// Leaf-specific introspection payload.
///
/// This duplicates [`LeafIntrospectionSummary`] intentionally because the leaf-only response is
/// a distinct wire payload from the endpoint-wide discovery response.
///
/// # Example
/// ```rust
/// use unshell::protocol::LeafIntrospection;
/// let payload = LeafIntrospection {
///     leaf_name: "org.example.v1.echo".into(),
///     procedures: vec!["org.example.v1.echo.invoke".into()],
/// };
/// assert_eq!(payload.leaf_name, "org.example.v1.echo");
/// ```
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct LeafIntrospection {
    /// Canonical dotted leaf identifier.
    pub leaf_name: String,
    /// Exhaustive canonical procedure ids currently exposed by the leaf.
    pub procedures: Vec<String>,
}
