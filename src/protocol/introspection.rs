//! Required introspection payloads for discovery.

use alloc::{string::String, vec::Vec};
use rkyv::{Archive, Deserialize, Serialize};

/// Reserved procedure id for protocol introspection.
///
/// The protocol uses the empty string here so discovery traffic stays outside the normal
/// application procedure namespace. [`crate::protocol::validate_procedure_id`] reserves that
/// value exclusively for introspection.
pub const INTROSPECTION_PROCEDURE_ID: &str = "";

/// Endpoint-wide introspection payload.
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct EndpointIntrospection {
    /// Direct child endpoint segment names hosted immediately below this endpoint.
    pub sub_endpoints: Vec<String>,
    /// Leaf summaries hosted directly at this endpoint.
    pub leaves: Vec<LeafIntrospectionSummary>,
}

/// Shared per-leaf discovery record.
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
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct LeafIntrospection {
    /// Canonical dotted leaf identifier.
    pub leaf_name: String,
    /// Exhaustive canonical procedure ids currently exposed by the leaf.
    pub procedures: Vec<String>,
}
