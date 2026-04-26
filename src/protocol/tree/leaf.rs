//! Application-facing leaf metadata helpers.
//!
//! The protocol runtime itself only knows about `LeafSpec` metadata and validated
//! `LocalEvent` delivery. `ProtocolLeaf` owns the canonical dotted leaf id, while
//! `CallProcedures` owns generated procedure ids and initial call dispatch.

use alloc::{string::String, vec::Vec};

use super::LeafSpec;

/// Static metadata for one application-defined protocol leaf.
pub trait ProtocolLeaf {
    /// Returns the canonical dotted leaf name hosted by this type.
    fn leaf_name() -> String;
}

/// Generated call metadata and initial `Call` dispatch for one leaf.
pub trait CallProcedures: ProtocolLeaf {
    /// Leaf-specific error surfaced when generated call dispatch fails.
    type Error;

    /// Returns the local procedure suffixes supported by this leaf.
    fn procedure_suffixes() -> &'static [&'static str];

    /// Resolves one local procedure suffix to its full canonical `procedure_id`.
    fn procedure_id(suffix: &str) -> Option<String> {
        if !Self::procedure_suffixes().contains(&suffix) {
            return None;
        }

        let mut procedure_id = Self::leaf_name();
        procedure_id.push('.');
        procedure_id.push_str(suffix);
        Some(procedure_id)
    }

    /// Returns the full canonical `procedure_id` values supported by this leaf.
    fn procedure_ids() -> Vec<String> {
        Self::procedure_suffixes()
            .iter()
            .filter_map(|suffix| Self::procedure_id(suffix))
            .collect()
    }

    /// Materializes the runtime leaf metadata consumed by `ProtocolEndpoint`.
    fn leaf_spec() -> LeafSpec {
        LeafSpec {
            name: Self::leaf_name(),
            procedures: Self::procedure_ids(),
        }
    }

    /// Dispatches one initial `Call` that targeted this leaf.
    ///
    /// Implementations may assume the endpoint already proved the call targets this leaf.
    /// They are still responsible for decoding the typed input payload and deciding which local
    /// procedure suffix should run.
    fn dispatch_call(
        &mut self,
        call: crate::protocol::tree::IncomingCall,
    ) -> Result<crate::protocol::tree::CallReply, crate::protocol::tree::DispatchError<Self::Error>>;
}

/// Builds one canonical dotted leaf id from crate-local metadata plus optional
/// user overrides.
///
/// Rationale: derive macros cannot reliably inspect Cargo workspace metadata, but
/// they can always access the current package name, module path, crate version,
/// and Rust type name at the expansion site. This helper normalizes those inputs
/// into one deterministic dotted identifier without leaking Rust separators or
/// casing into protocol-visible names. Deterministic is not the same as stable
/// across refactors, so shipped protocol surfaces should prefer explicit `id`
/// overrides.
///
/// # Example
/// ```rust
/// use unshell::protocol::tree::derive_leaf_name;
///
/// let leaf = derive_leaf_name(
///     "unshell-core",
///     "0",
///     "1",
///     "0",
///     "unshell_core::examples::demo_shell",
///     "ShellLeaf",
///     None,
///     None,
///     None,
///     None,
///     None,
/// );
/// assert_eq!(leaf, "unshell_core.unshell_core.v0_1_0.examples.demo_shell.shell_leaf");
/// ```
#[allow(clippy::too_many_arguments)]
// This helper mirrors derive-macro inputs directly so callers do not have to allocate an
// intermediate metadata struct just to compute one deterministic protocol identifier.
pub fn derive_leaf_name(
    package_name: &str,
    version_major: &str,
    version_minor: &str,
    version_patch: &str,
    module_path: &str,
    type_name: &str,
    org: Option<&str>,
    product: Option<&str>,
    version: Option<&str>,
    leaf_name: Option<&str>,
    id: Option<&str>,
) -> String {
    if let Some(id) = id.filter(|value| !value.is_empty()) {
        return String::from(id);
    }

    let package_segment = normalize_leaf_segment(package_name);
    let mut segments = Vec::new();
    segments.push(normalize_leaf_segment(org.unwrap_or(package_name)));
    segments.push(normalize_leaf_segment(product.unwrap_or(package_name)));
    segments.push(normalize_version_segment(version.unwrap_or(
        &alloc::format!("v{}_{}_{}", version_major, version_minor, version_patch),
    )));

    if let Some(leaf_name) = leaf_name.filter(|value| !value.is_empty()) {
        segments.extend(split_leaf_path(leaf_name));
    } else {
        let mut module_segments = module_path
            .split("::")
            .map(normalize_leaf_segment)
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<_>>();
        if module_segments
            .first()
            .is_some_and(|segment| segment == &package_segment)
        {
            module_segments.remove(0);
        }
        segments.extend(module_segments);
        segments.push(normalize_leaf_segment(type_name));
    }

    segments.join(".")
}

fn split_leaf_path(value: &str) -> Vec<String> {
    value
        .split('.')
        .map(normalize_leaf_segment)
        .filter(|segment| !segment.is_empty())
        .collect()
}

fn normalize_version_segment(value: &str) -> String {
    let normalized = normalize_leaf_segment(value);
    if normalized.starts_with('v') && normalized.len() > 1 {
        normalized
    } else {
        alloc::format!("v{}", normalized)
    }
}

fn normalize_leaf_segment(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    let mut previous_was_separator = false;

    for character in value.chars() {
        if character.is_ascii_uppercase() {
            // Preserve CamelCase word boundaries in a snake_case protocol identifier.
            if !normalized.is_empty() && !previous_was_separator {
                normalized.push('_');
            }
            normalized.push(character.to_ascii_lowercase());
            previous_was_separator = false;
            continue;
        }

        if character.is_ascii_lowercase() || character.is_ascii_digit() {
            normalized.push(character);
            previous_was_separator = false;
            continue;
        }

        if !normalized.is_empty() && !previous_was_separator {
            normalized.push('_');
            previous_was_separator = true;
        }
    }

    while normalized.ends_with('_') {
        normalized.pop();
    }

    if normalized.is_empty() {
        // Protocol identifiers still need a stable non-empty placeholder when user input is all
        // punctuation or whitespace.
        String::from("leaf")
    } else {
        normalized
    }
}

#[cfg(test)]
mod tests {
    use super::derive_leaf_name;

    #[test]
    fn derive_leaf_name_normalizes_inputs_into_dotted_segments() {
        assert_eq!(
            derive_leaf_name(
                "unshell-core",
                "0",
                "1",
                "0",
                "unshell_core::examples::demo_shell",
                "ShellLeaf",
                None,
                None,
                None,
                None,
                None,
            ),
            "unshell_core.unshell_core.v0_1_0.examples.demo_shell.shell_leaf"
        );
    }

    #[test]
    fn derive_leaf_name_applies_partial_overrides() {
        assert_eq!(
            derive_leaf_name(
                "unshell-core",
                "0",
                "1",
                "0",
                "unshell_core::examples::demo_shell",
                "ShellLeaf",
                Some("org"),
                Some("product"),
                Some("v1.2.3.4"),
                Some("echo.shell"),
                None,
            ),
            "org.product.v1_2_3_4.echo.shell"
        );
    }

    #[test]
    fn derive_leaf_name_id_override_wins() {
        assert_eq!(
            derive_leaf_name(
                "unshell-core",
                "0",
                "1",
                "0",
                "unshell_core::examples::demo_shell",
                "ShellLeaf",
                Some("org"),
                Some("product"),
                Some("v1"),
                Some("echo"),
                Some("org.example.v1.echo.abc"),
            ),
            "org.example.v1.echo.abc"
        );
    }
}
