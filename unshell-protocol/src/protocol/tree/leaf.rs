//! Application-facing leaf metadata helpers.
//!
//! The protocol runtime itself only knows about `LeafSpec` metadata and validated
//! `LocalEvent` delivery. `ProtocolLeaf` owns canonical identity, `LeafDeclaration`
//! owns the compile-time procedure inventory for one leaf surface, and
//! `CallProcedures` adds local call dispatch on top of that inventory.

use alloc::{string::String, vec::Vec};

use crate::protocol::FrameBytes;

use super::{ChildRoute, LeafSpec, ProtocolEndpoint};

/// Static metadata for one application-defined protocol leaf.
///
/// This exists so runtime code can ask one type for its canonical dotted leaf id without knowing
/// any of that leaf's call-dispatch details.
///
/// # Example
/// ```rust
/// use unshell::protocol::tree::ProtocolLeaf;
/// struct ExampleLeaf;
/// impl ProtocolLeaf for ExampleLeaf {
///     fn leaf_name() -> String { "org.example.v1.echo".into() }
/// }
/// assert_eq!(ExampleLeaf::leaf_name(), "org.example.v1.echo");
/// ```
pub trait ProtocolLeaf {
    /// Returns the canonical dotted leaf name hosted by this type.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::ProtocolLeaf;
    /// struct ExampleLeaf;
    /// impl ProtocolLeaf for ExampleLeaf {
    ///     fn leaf_name() -> String { "org.example.v1.echo".into() }
    /// }
    /// assert!(ExampleLeaf::leaf_name().starts_with("org.example"));
    /// ```
    fn leaf_name() -> String;
}

/// Compile-time declaration metadata for one leaf surface.
///
/// What it is: a trait for types that can describe the complete protocol-visible
/// surface of one leaf at compile time.
///
/// Why it exists: endpoint construction should not need handwritten procedure
/// lists. A leaf declaration can generate the canonical suffix inventory once and
/// let both endpoint and TUI host types reuse it.
///
/// # Example
/// ```rust
/// use unshell::protocol::tree::{LeafDeclaration, ProtocolLeaf};
/// struct ExampleLeaf;
/// impl ProtocolLeaf for ExampleLeaf {
///     fn leaf_name() -> String { "org.example.v1.echo".into() }
/// }
/// impl LeafDeclaration for ExampleLeaf {
///     fn procedure_suffixes() -> &'static [&'static str] { &["invoke"] }
/// }
/// assert_eq!(ExampleLeaf::leaf_spec().procedures, vec![String::from("org.example.v1.echo.invoke")]);
/// ```
pub trait LeafDeclaration: ProtocolLeaf {
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
}

/// Returns the canonical `LeafSpec` for one concrete leaf host value.
///
/// What it is: a tiny typed helper that uses a host value only for type inference.
///
/// Why it exists: endpoint-construction macros can accept ordinary host expressions like
/// `RemoteShell::default()` and still derive the compile-time `LeafSpec` without the caller
/// spelling the leaf type twice.
///
/// # Example
/// ```rust
/// use unshell::protocol::tree::{LeafDeclaration, ProtocolLeaf, leaf_spec_of};
/// struct ExampleLeaf;
/// impl ProtocolLeaf for ExampleLeaf {
///     fn leaf_name() -> String { "org.example.v1.echo".into() }
/// }
/// impl LeafDeclaration for ExampleLeaf {
///     fn procedure_suffixes() -> &'static [&'static str] { &["invoke"] }
/// }
/// let spec = leaf_spec_of(&ExampleLeaf);
/// assert_eq!(spec.name, "org.example.v1.echo");
/// ```
pub fn leaf_spec_of<L>(_: &L) -> LeafSpec
where
    L: LeafDeclaration,
{
    L::leaf_spec()
}

/// Declares that one host struct is bound to one compile-time leaf declaration.
///
/// What it is: a trait that links a concrete host type, such as an endpoint or
/// TUI struct, back to the declaration that owns its shared protocol metadata.
///
/// Why it exists: endpoint and TUI hosts often need different state and behavior,
/// but they should still share one canonical leaf identity and procedure list.
///
/// # Example
/// ```rust
/// use unshell::protocol::tree::{LeafBinding, LeafDeclaration, ProtocolLeaf};
/// struct ExampleDecl;
/// impl ProtocolLeaf for ExampleDecl {
///     fn leaf_name() -> String { "org.example.v1.echo".into() }
/// }
/// impl LeafDeclaration for ExampleDecl {
///     fn procedure_suffixes() -> &'static [&'static str] { &["invoke"] }
/// }
/// struct ExampleHost;
/// impl ProtocolLeaf for ExampleHost {
///     fn leaf_name() -> String { ExampleDecl::leaf_name() }
/// }
/// impl LeafBinding for ExampleHost {
///     type Declaration = ExampleDecl;
/// }
/// assert_eq!(<ExampleHost as LeafBinding>::Declaration::leaf_name(), "org.example.v1.echo");
/// ```
pub trait LeafBinding: ProtocolLeaf {
    /// Shared declaration that owns the canonical metadata for this host type.
    type Declaration: ProtocolLeaf;
}

/// Generated call metadata and initial `Call` dispatch for one leaf.
///
/// This exists so one leaf type can advertise which procedure suffixes it serves and convert an
/// opening protocol `Call` into leaf-local behavior.
///
/// # Example
/// ```rust
/// use unshell::protocol::tree::{CallProcedures, DispatchError, IncomingCall, ProtocolLeaf};
/// struct ExampleLeaf;
/// impl ProtocolLeaf for ExampleLeaf {
///     fn leaf_name() -> String { "org.example.v1.echo".into() }
/// }
/// impl CallProcedures for ExampleLeaf {
///     type Error = core::convert::Infallible;
///     fn procedure_suffixes() -> &'static [&'static str] { &["invoke"] }
///     fn dispatch_call(&mut self, _endpoint: &mut unshell::protocol::tree::ProtocolEndpoint, _call: IncomingCall) -> Result<unshell::protocol::tree::CallReply, DispatchError<Self::Error>> {
///         Ok(unshell::protocol::tree::CallReply::NoReply)
///     }
/// }
/// assert_eq!(ExampleLeaf::procedure_id("invoke").unwrap(), "org.example.v1.echo.invoke");
/// ```
pub trait CallProcedures: LeafDeclaration {
    /// Leaf-specific error surfaced when generated call dispatch fails.
    type Error;

    /// Dispatches one initial `Call` that targeted this leaf.
    ///
    /// Implementations may assume the endpoint already proved the call targets this leaf.
    /// They are still responsible for decoding the typed input payload and deciding which local
    /// procedure suffix should run.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::{CallProcedures, DispatchError, IncomingCall, ProtocolLeaf};
    /// struct ExampleLeaf;
    /// impl ProtocolLeaf for ExampleLeaf { fn leaf_name() -> String { "org.example.v1.echo".into() } }
/// impl CallProcedures for ExampleLeaf {
///     type Error = core::convert::Infallible;
///     fn procedure_suffixes() -> &'static [&'static str] { &["invoke"] }
///     fn dispatch_call(&mut self, _endpoint: &mut unshell::protocol::tree::ProtocolEndpoint, _call: IncomingCall) -> Result<unshell::protocol::tree::CallReply, DispatchError<Self::Error>> {
///         Ok(unshell::protocol::tree::CallReply::NoReply)
///     }
/// }
    /// # let _ = ExampleLeaf;
    /// ```
    fn dispatch_call(
        &mut self,
        endpoint: &mut ProtocolEndpoint,
        call: crate::protocol::tree::IncomingCall,
    ) -> Result<crate::protocol::tree::CallReply, crate::protocol::tree::DispatchError<Self::Error>>;
}

/// Router-facing transport hooks for leaves that own parent/child connections.
///
/// What it is: an opt-in trait for leaves that want to act as the transport layer
/// for one endpoint's forwarded traffic.
///
/// Why it exists: ordinary leaves only need validated local events, but a router
/// leaf also needs to know its active parent/children and where to physically send
/// frames chosen by the endpoint's routing logic.
///
/// # Example
/// ```rust
/// use unshell::protocol::FrameBytes;
/// use unshell::protocol::tree::{ChildRoute, RouterLeaf};
/// #[derive(Default)]
/// struct DemoRouter {
///     parent: Option<Vec<String>>,
///     children: Vec<ChildRoute>,
/// }
/// impl unshell::protocol::tree::ProtocolLeaf for DemoRouter {
///     fn leaf_name() -> String { "org.example.v1.router".into() }
/// }
/// impl RouterLeaf for DemoRouter {
///     type RouteError = core::convert::Infallible;
///
///     fn parent_path(&self) -> Option<&[String]> { self.parent.as_deref() }
///     fn child_routes(&self) -> &[ChildRoute] { &self.children }
///     fn route_to_parent(&mut self, _local_path: &[String], _frame: FrameBytes) -> Result<(), Self::RouteError> { Ok(()) }
///     fn route_to_child(&mut self, _child_path: &[String], _frame: FrameBytes) -> Result<(), Self::RouteError> { Ok(()) }
/// }
/// ```
pub trait RouterLeaf: ProtocolLeaf {
    /// Transport-specific error surfaced while handing a frame to the chosen link.
    type RouteError;

    /// Returns the currently connected direct parent path, if any.
    fn parent_path(&self) -> Option<&[String]>;

    /// Returns the currently connected direct child routes.
    fn child_routes(&self) -> &[ChildRoute];

    /// Sends one routed frame toward the direct parent connection.
    fn route_to_parent(
        &mut self,
        local_path: &[String],
        frame: FrameBytes,
    ) -> Result<(), Self::RouteError>;

    /// Sends one routed frame toward the chosen direct child connection.
    fn route_to_child(
        &mut self,
        child_path: &[String],
        frame: FrameBytes,
    ) -> Result<(), Self::RouteError>;
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
        // The package-derived prefix already names the crate/product portion of the identifier, so
        // strip the same leading segment from `module_path` when it would otherwise duplicate it.
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
    use alloc::string::String;

    use super::{LeafBinding, LeafDeclaration, ProtocolLeaf, derive_leaf_name};

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

    #[test]
    fn bound_hosts_can_share_one_declaration() {
        struct SharedDecl;
        impl ProtocolLeaf for SharedDecl {
            fn leaf_name() -> String {
                String::from("org.example.v1.echo")
            }
        }
        impl LeafDeclaration for SharedDecl {
            fn procedure_suffixes() -> &'static [&'static str] {
                &["invoke"]
            }
        }

        struct Host;
        impl ProtocolLeaf for Host {
            fn leaf_name() -> String {
                SharedDecl::leaf_name()
            }
        }
        impl LeafBinding for Host {
            type Declaration = SharedDecl;
        }

        assert_eq!(
            <Host as LeafBinding>::Declaration::leaf_spec().name,
            "org.example.v1.echo"
        );
    }
}
