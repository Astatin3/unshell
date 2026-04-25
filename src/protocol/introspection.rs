//! Required introspection payloads for discovery.

use alloc::{string::String, vec::Vec};
use rkyv::{Archive, Deserialize, Serialize};

/// Reserved procedure id for protocol introspection.
pub const INTROSPECTION_PROCEDURE_ID: &str = "";

/// Endpoint-wide introspection payload.
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct EndpointIntrospection {
    /// Direct child path segments currently registered under this endpoint.
    pub sub_endpoints: Vec<String>,
    /// Hosted leaves and their supported procedures.
    pub leaves: Vec<LeafIntrospectionSummary>,
}

/// Shared per-leaf discovery record.
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct LeafIntrospectionSummary {
    /// Local leaf name.
    pub leaf_name: String,
    /// Canonical procedure identifiers supported by the leaf.
    pub procedures: Vec<String>,
}

/// Leaf-specific introspection payload.
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct LeafIntrospection {
    /// Local leaf name.
    pub leaf_name: String,
    /// Canonical procedure identifiers supported by the leaf.
    pub procedures: Vec<String>,
}
