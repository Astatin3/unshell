//! Path routing helpers and explicit enum tree declarations.
//!
//! Routing follows a longest-prefix rule over endpoint paths. Each endpoint boundary can compile
//! its children into a small trie so repeated route decisions do not need to scan every child.

use alloc::{collections::BTreeMap, string::String, vec, vec::Vec};

/// Explicit tree declaration used for configuration and tests.
///
/// This models one protocol tree declaratively so callers can derive endpoint paths, leaf
/// inventory, or test fixtures without first constructing live endpoints.
///
/// # Example
/// ```rust
/// use unshell::protocol::tree::{LeafNode, TreeNode};
/// let tree = TreeNode::Root {
///     children: vec![TreeNode::Endpoint {
///         segment: "worker".into(),
///         leaves: vec![LeafNode {
///             name: "service".into(),
///             procedures: vec!["example.service.v1.invoke".into()],
///         }],
///         children: Vec::new(),
///     }],
/// };
/// assert_eq!(tree.paths().len(), 2);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeNode {
    /// The protocol root. Its path is always empty.
    Root {
        /// Direct child endpoints hosted below the root.
        children: Vec<Self>,
    },
    /// An addressable endpoint segment in the tree.
    Endpoint {
        /// Path segment contributed by this endpoint.
        segment: String,
        /// Leaves hosted directly at this endpoint.
        leaves: Vec<LeafNode>,
        /// Direct child endpoints hosted below this endpoint.
        children: Vec<Self>,
    },
}

/// Leaf declaration used inside the explicit tree enum.
///
/// This exists so declarative trees can describe the leaves hosted at one endpoint without
/// constructing the full runtime state machine.
///
/// # Example
/// ```rust
/// use unshell::protocol::tree::LeafNode;
/// let leaf = LeafNode {
///     name: "service".into(),
///     procedures: vec!["example.service.v1.invoke".into()],
/// };
/// assert_eq!(leaf.name, "service");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeafNode {
    /// Leaf name local to an endpoint path.
    pub name: String,
    /// Procedures served by this leaf.
    pub procedures: Vec<String>,
}

impl TreeNode {
    /// Flattens the explicit tree into the set of endpoint paths it declares.
    ///
    /// The returned list always includes the protocol root as `[]`.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::TreeNode;
    /// let tree = TreeNode::Root {
    ///     children: vec![TreeNode::Endpoint {
    ///         segment: "worker".into(),
    ///         leaves: Vec::new(),
    ///         children: Vec::new(),
    ///     }],
    /// };
    /// assert_eq!(tree.paths(), vec![Vec::<String>::new(), vec!["worker".into()]]);
    /// ```
    pub fn paths(&self) -> Vec<Vec<String>> {
        let mut paths = Vec::new();
        self.collect_paths(&[], &mut paths);
        paths
    }

    fn collect_paths(&self, prefix: &[String], paths: &mut Vec<Vec<String>>) {
        match self {
            Self::Root { children } => {
                paths.push(Vec::new());
                for child in children {
                    // Root always restarts collection from the empty path.
                    child.collect_paths(&[], paths);
                }
            }
            Self::Endpoint {
                segment, children, ..
            } => {
                let mut next = prefix.to_vec();
                next.push(segment.clone());
                paths.push(next.clone());
                for child in children {
                    child.collect_paths(&next, paths);
                }
            }
        }
    }
}

/// Longest-prefix route decision.
///
/// Each decision is evaluated from one endpoint's perspective after comparing its own path and
/// compiled child subtree against the destination path.
///
/// # Example
/// ```rust
/// use unshell::protocol::tree::RouteDecision;
/// let route = RouteDecision::Child(0);
/// assert!(matches!(route, RouteDecision::Child(0)));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteDecision {
    /// Forward to the child at the given local child index.
    Child(usize),
    /// Deliver locally at this endpoint.
    Local,
    /// Forward upward because the destination is outside the local subtree.
    Parent,
    /// Drop because no local, child, or parent route applies.
    Drop,
}

/// One compiled routing table for one endpoint boundary.
///
/// This exists so repeated route lookups can reuse one longest-prefix trie instead of scanning
/// every child path on every packet.
///
/// # Example
/// ```rust
/// use unshell::protocol::tree::{CompiledRoutes, RouteDecision};
/// let routes = CompiledRoutes::new(&["root".into()], &[vec!["root".into(), "worker".into()]], true);
/// assert_eq!(routes.route(&["root".into(), "worker".into(), "job".into()]), RouteDecision::Child(0));
/// ```
#[derive(Debug, Clone, Default)]
pub struct CompiledRoutes {
    local_path: Vec<String>,
    has_parent: bool,
    nodes: Vec<RouteTrieNode>,
}

#[derive(Debug, Clone, Default)]
struct RouteTrieNode {
    /// Child selected when traversal stops exactly at this trie node.
    best_child: Option<usize>,
    edges: BTreeMap<String, usize>,
}

impl CompiledRoutes {
    /// Compiles child endpoint paths into a trie rooted at `local_path`.
    ///
    /// Only strict descendants of `local_path` participate in the compiled trie. Paths outside
    /// the local subtree, or equal to `local_path` itself, are ignored.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::CompiledRoutes;
    /// let routes = CompiledRoutes::new(
    ///     &["root".into()],
    ///     &[
    ///         vec!["root".into(), "worker".into()],
    ///         vec!["other".into()],
    ///     ],
    ///     true,
    /// );
    /// assert_eq!(routes.route(&["root".into(), "worker".into()]), unshell::protocol::tree::RouteDecision::Child(0));
    /// ```
    #[must_use]
    pub fn new(local_path: &[String], child_paths: &[Vec<String>], has_parent: bool) -> Self {
        let mut routes = Self {
            local_path: local_path.to_vec(),
            has_parent,
            nodes: vec![RouteTrieNode::default()],
        };

        for (index, child_path) in child_paths.iter().enumerate() {
            routes.insert_child(index, child_path);
        }

        routes
    }

    fn insert_child(&mut self, index: usize, child_path: &[String]) {
        if !is_prefix(&self.local_path, child_path) || child_path.len() <= self.local_path.len() {
            return;
        }

        // Store only strict descendants. The terminal node records which direct child owns that
        // descendant boundary so later lookups can recover the longest matching child index.
        let mut node_index = 0usize;
        for segment in &child_path[self.local_path.len()..] {
            let next_index = if let Some(next_index) = self.nodes[node_index].edges.get(segment) {
                *next_index
            } else {
                let next_index = self.nodes.len();
                self.nodes.push(RouteTrieNode::default());
                self.nodes[node_index]
                    .edges
                    .insert(segment.clone(), next_index);
                next_index
            };
            node_index = next_index;
        }

        self.nodes[node_index].best_child = Some(index);
    }

    /// Resolves `dst_path` using the compiled longest-prefix trie.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::{CompiledRoutes, RouteDecision};
    /// let routes = CompiledRoutes::new(&["root".into()], &[vec!["root".into(), "worker".into()]], true);
    /// assert_eq!(routes.route(&["root".into(), "worker".into()]), RouteDecision::Child(0));
    /// assert_eq!(routes.route(&["root".into()]), RouteDecision::Local);
    /// assert_eq!(routes.route(&["elsewhere".into()]), RouteDecision::Parent);
    /// ```
    #[must_use]
    pub fn route(&self, dst_path: &[String]) -> RouteDecision {
        if !is_prefix(&self.local_path, dst_path) {
            return if self.has_parent {
                RouteDecision::Parent
            } else {
                RouteDecision::Drop
            };
        }

        let mut best_child = None;
        let mut node_index = 0usize;
        for segment in &dst_path[self.local_path.len()..] {
            let Some(next_index) = self.nodes[node_index].edges.get(segment) else {
                break;
            };
            node_index = *next_index;
            if let Some(index) = self.nodes[node_index].best_child {
                // Keep the deepest matching child seen so far; if traversal breaks later, the
                // protocol still routes to the longest matching descendant boundary.
                best_child = Some(index);
            }
        }

        if let Some(index) = best_child {
            return RouteDecision::Child(index);
        }
        if self.local_path == dst_path {
            return RouteDecision::Local;
        }
        RouteDecision::Drop
    }
}

/// Returns `true` if `prefix` is a path prefix of `path`.
///
/// This exists as the shared path-comparison primitive for both declarative tree processing and
/// runtime route compilation.
///
/// # Example
/// ```rust
/// use unshell::protocol::tree::is_prefix;
/// assert!(is_prefix(&["root".into()], &["root".into(), "worker".into()]));
/// assert!(!is_prefix(&["worker".into()], &["root".into(), "worker".into()]));
/// ```
pub fn is_prefix(prefix: &[String], path: &[String]) -> bool {
    prefix.len() <= path.len()
        && prefix
            .iter()
            .zip(path.iter())
            .all(|(left, right)| left == right)
}
/// Trait for resolving a destination path to a routing decision.
///
/// The default policy is longest-prefix routing: exact matches stay local, the deepest matching
/// descendant wins for child forwarding, destinations outside the local subtree go to the parent
/// when one exists, and everything else drops.
///
/// # Example
/// ```rust
/// use unshell::protocol::tree::{DefaultRouteProvider, RouteProvider};
/// let provider = DefaultRouteProvider;
/// let route = provider.route_destination(
///     &["root".into()],
///     [vec!["root".into(), "worker".into()]],
///     true,
///     &["root".into(), "worker".into()],
/// );
/// assert!(matches!(route, unshell::protocol::tree::RouteDecision::Child(0)));
/// ```
pub trait RouteProvider {
    /// Returns the route decision for `dst_path` from the perspective of `local_path`.
    fn route_destination<I>(
        &self,
        local_path: &[String],
        child_paths: I,
        has_parent: bool,
        dst_path: &[String],
    ) -> RouteDecision
    where
        I: IntoIterator,
        I::Item: AsRef<[String]>;
}

/// Default routing implementation using the protocol's longest-prefix rule.
///
/// This exists as the stateless policy object behind the free [`route_destination`] helper and
/// as a customization seam for tests or alternate routing strategies.
///
/// # Example
/// ```rust
/// use unshell::protocol::tree::{DefaultRouteProvider, RouteProvider};
/// let provider = DefaultRouteProvider;
/// let route = provider.route_destination(&[], [vec!["worker".into()]], false, &["worker".into()]);
/// assert!(matches!(route, unshell::protocol::tree::RouteDecision::Child(0)));
/// ```
pub struct DefaultRouteProvider;

impl RouteProvider for DefaultRouteProvider {
    fn route_destination<I>(
        &self,
        local_path: &[String],
        child_paths: I,
        has_parent: bool,
        dst_path: &[String],
    ) -> RouteDecision
    where
        I: IntoIterator,
        I::Item: AsRef<[String]>,
    {
        let child_paths = child_paths
            .into_iter()
            .map(|child| child.as_ref().to_vec())
            .collect::<Vec<_>>();
        CompiledRoutes::new(local_path, &child_paths, has_parent).route(dst_path)
    }
}

/// Resolves `dst_path` with the default longest-prefix route provider.
///
/// Exact matches return [`RouteDecision::Local`]. Destinations outside the local subtree return
/// [`RouteDecision::Parent`] when `has_parent` is `true`, otherwise [`RouteDecision::Drop`].
///
/// # Example
/// ```rust
/// use unshell::protocol::tree::{RouteDecision, route_destination};
/// let route = route_destination(&[], [vec!["worker".into()]], false, &["worker".into()]);
/// assert_eq!(route, RouteDecision::Child(0));
/// ```
pub fn route_destination<I>(
    local_path: &[String],
    child_paths: I,
    has_parent: bool,
    dst_path: &[String],
) -> RouteDecision
where
    I: IntoIterator,
    I::Item: AsRef<[String]>,
{
    DefaultRouteProvider.route_destination(local_path, child_paths, has_parent, dst_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{string::String, vec};

    #[test]
    fn longest_prefix_wins() {
        let provider = DefaultRouteProvider;
        let children = vec![
            vec![String::from("a")],
            vec![String::from("a"), String::from("b")],
        ];
        assert_eq!(
            provider.route_destination(
                &Vec::<String>::new(),
                children,
                false,
                &[String::from("a"), String::from("b"), String::from("c")]
            ),
            RouteDecision::Child(1)
        );
    }

    #[test]
    fn compiled_routes_choose_longest_prefix_without_child_scan() {
        let table = CompiledRoutes::new(
            &[String::from("a")],
            &[
                vec![String::from("a"), String::from("b")],
                vec![String::from("a"), String::from("x")],
            ],
            true,
        );

        assert_eq!(
            table.route(&[String::from("a"), String::from("b"), String::from("c")]),
            RouteDecision::Child(0)
        );
        assert_eq!(table.route(&[String::from("z")]), RouteDecision::Parent);
    }

    #[test]
    fn tree_enum_flattens_paths() {
        let tree = TreeNode::Root {
            children: vec![TreeNode::Endpoint {
                segment: String::from("a"),
                leaves: Vec::new(),
                children: vec![TreeNode::Endpoint {
                    segment: String::from("b"),
                    leaves: Vec::new(),
                    children: Vec::new(),
                }],
            }],
        };

        assert_eq!(
            tree.paths(),
            vec![
                Vec::<String>::new(),
                vec![String::from("a")],
                vec![String::from("a"), String::from("b")],
            ]
        );
    }
}
