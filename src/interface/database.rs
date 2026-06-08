use alloc::vec::Vec;

/// Namespaced blob storage for serialized interface state.
///
/// This is the whole persistence contract. A generated leaf writes serialized session
/// objects under deterministic binary namespaces, then scans those namespaces later to
/// reconstruct historical sessions using the matching session type. The database is
/// deliberately unaware of packets and audit events so cybersecurity audit data can
/// be meaningful leaf-owned state instead of a lossy transport log.
pub trait InterfaceDatabase {
    /// Stores one opaque value under `namespace` and `key`.
    ///
    /// Implementations may replace an existing value with the same namespace/key or
    /// version it internally. The core crate only relies on a later `scan` returning
    /// values that were written for the requested namespace.
    fn put(&mut self, namespace: &[u8], key: &[u8], value: &[u8]);

    /// Returns every key/value pair currently visible under `namespace`.
    ///
    /// Returning owned bytes keeps this trait object-safe and simple for both in-memory
    /// and external database implementations. Higher-performance stores can add their
    /// own adapter methods later without changing the leaf contract.
    fn scan(&mut self, namespace: &[u8]) -> Vec<(Vec<u8>, Vec<u8>)>;
}
