//! Required introspection payloads for discovery.

use alloc::{string::String, vec::Vec};
use rkyv::{Archive, Deserialize, Serialize};

/// Reserved procedure id for protocol introspection.
pub const INTROSPECTION_PROCEDURE_ID: &str = "";

/// Endpoint-wide introspection payload.
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct EndpointIntrospection {
    pub sub_endpoints: Vec<String>,
    pub leaves: Vec<LeafIntrospectionSummary>,
}

/// Shared per-leaf discovery record.
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct LeafIntrospectionSummary {
    pub leaf_name: String,
    pub procedures: Vec<String>,
}

/// Leaf-specific introspection payload.
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct LeafIntrospection {
    pub leaf_name: String,
    pub procedures: Vec<String>,
}
