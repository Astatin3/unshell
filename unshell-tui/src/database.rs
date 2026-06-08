use std::collections::BTreeMap;

use unshell::interface::InterfaceDatabase;

/// In-memory implementation of the core interface blob store.
///
/// This database is intentionally small and deterministic. It is suitable for tests,
/// demos, and single-process TUI sessions where serialized leaf state only needs to
/// live for the lifetime of the operator tool. Production audit tooling can implement
/// [`InterfaceDatabase`] with a durable store while preserving the same opaque
/// namespace/key/value contract.
#[derive(Debug, Default, Clone)]
pub struct MemoryInterfaceDatabase {
    values: BTreeMap<(Vec<u8>, Vec<u8>), Vec<u8>>,
}

impl MemoryInterfaceDatabase {
    /// Creates an empty database.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the number of stored blobs across all namespaces.
    ///
    /// This is a TUI/testing convenience rather than part of the core trait because
    /// leaves should only rely on namespace scans, not global database inspection.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns `true` when no blobs have been stored.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

impl InterfaceDatabase for MemoryInterfaceDatabase {
    fn put(&mut self, namespace: &[u8], key: &[u8], value: &[u8]) {
        self.values
            .insert((namespace.to_vec(), key.to_vec()), value.to_vec());
    }

    fn scan(&mut self, namespace: &[u8]) -> Vec<(Vec<u8>, Vec<u8>)> {
        self.values
            .iter()
            .filter_map(|((stored_namespace, stored_key), stored_value)| {
                // The namespace is caller-owned bytes. Equality is the only contract;
                // the database deliberately does not decode leaf or procedure ids.
                (stored_namespace.as_slice() == namespace)
                    .then(|| (stored_key.clone(), stored_value.clone()))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use unshell::interface::{hook_key, session_namespace};

    use super::*;

    #[test]
    fn put_replaces_existing_namespace_key_blob() {
        let namespace = session_namespace(7, 11);
        let key = hook_key(3);
        let mut database = MemoryInterfaceDatabase::new();

        database.put(&namespace, &key, b"old");
        database.put(&namespace, &key, b"new");

        assert_eq!(database.len(), 1);
        assert_eq!(database.scan(&namespace), vec![(key, b"new".to_vec())]);
    }

    #[test]
    fn scan_only_returns_requested_namespace() {
        let first_namespace = session_namespace(1, 2);
        let second_namespace = session_namespace(1, 3);
        let mut database = MemoryInterfaceDatabase::new();

        database.put(&first_namespace, &hook_key(1), b"first");
        database.put(&second_namespace, &hook_key(1), b"second");

        assert_eq!(
            database.scan(&first_namespace),
            vec![(hook_key(1), b"first".to_vec())]
        );
    }
}
