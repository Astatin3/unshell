use alloc::vec::Vec;

use crate::protocol::HookID;

const SESSION_NAMESPACE_TAG: u8 = 1;
const PROCEDURE_NAMESPACE_TAG: u8 = 2;

/// Builds the binary namespace for one generated session family.
///
/// The namespace is intentionally binary and fixed-width so databases do not need to
/// parse strings or trust display names. The generated macro uses the same function
/// for writes and reads, which keeps serialized sessions scoped to their owning leaf
/// and procedure id without creating a larger record schema.
pub fn session_namespace(leaf_id: u32, procedure_id: u32) -> Vec<u8> {
    namespace(SESSION_NAMESPACE_TAG, leaf_id, procedure_id)
}

/// Builds the binary namespace for one generated procedure family.
///
/// Procedure persistence is available for handwritten leaves and future generated
/// procedure state, but the first generated implementation focuses on sessions
/// because sessions are the long-lived objects users need to inspect historically.
pub fn procedure_namespace(leaf_id: u32, procedure_id: u32) -> Vec<u8> {
    namespace(PROCEDURE_NAMESPACE_TAG, leaf_id, procedure_id)
}

/// Builds the key for one hook-backed session object.
///
/// The hook id is part of the key, not a decoded database record. The matching session
/// type still owns the serialized value and can include the hook id inside its own
/// bytes if that helps later rendering or debugging.
pub fn hook_key(hook_id: HookID) -> Vec<u8> {
    hook_id.to_be_bytes().to_vec()
}

/// Builds a key for singleton state inside a namespace.
///
/// This helper is mainly for procedure or handwritten-leaf state where there is no
/// hook id. It avoids every caller inventing a different magic singleton key.
pub fn static_key(name: &[u8]) -> Vec<u8> {
    name.to_vec()
}

fn namespace(tag: u8, leaf_id: u32, procedure_id: u32) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(1 + 4 + 4);

    // Fixed ordering makes namespaces stable across architectures and database
    // implementations. Nothing in the database has to deserialize these bytes unless
    // it wants to offer debugging tools.
    bytes.push(tag);
    bytes.extend_from_slice(&leaf_id.to_be_bytes());
    bytes.extend_from_slice(&procedure_id.to_be_bytes());
    bytes
}
