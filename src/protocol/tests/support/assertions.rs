use crate::protocol::Endpoint;

/// Asserts that local hook state still contains `hook_id`.
///
/// Tests use this instead of open-coded map checks so every lifecycle assertion
/// explains the intended routing invariant when it fails.
pub(crate) fn assert_hook_present(endpoint: &Endpoint, hook_id: u16) {
    assert!(
        endpoint.has_hook(hook_id),
        "expected hook {hook_id} to remain registered"
    );
}

/// Asserts that local hook state no longer contains `hook_id`.
///
/// Upward `end_hook` packets are the only cases that should remove hook state;
/// downward and local packets with the same flag must leave hooks alone.
pub(crate) fn assert_hook_removed(endpoint: &Endpoint, hook_id: u16) {
    assert!(
        !endpoint.has_hook(hook_id),
        "expected hook {hook_id} to be cleaned up"
    );
}
