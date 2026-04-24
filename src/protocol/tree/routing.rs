//! Path routing helpers and explicit enum tree declarations.

use alloc::{string::String, vec::Vec};

/// Explicit test tree declaration used for configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeNode {
    /// The tree root.
    Root { children: Vec<Self> },
    /// A concrete endpoint in the tree.
    Endpoint {
        segment: String,
        leaves: Vec<LeafNode>,
        children: Vec<Self>,
    },
}

/// Leaf declaration used inside the explicit tree enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeafNode {
    /// Local leaf name.
    pub name: String,
    /// Supported procedures.
    pub procedures: Vec<String>,
}

impl TreeNode {
    /// Flattens the tree into absolute endpoint paths.
    pub fn paths(&self) -> Vec<Vec<String>> {
        let mut output = Vec::new();
        self.collect_paths(&[], &mut output);
        output
    }

    fn collect_paths(&self, prefix: &[String], output: &mut Vec<Vec<String>>) {
        match self {
            Self::Root { children } => {
                output.push(Vec::new());
                for child in children {
                    child.collect_paths(&[], output);
                }
            }
            Self::Endpoint {
                segment, children, ..
            } => {
                let mut next = prefix.to_vec();
                next.push(segment.clone());
                output.push(next.clone());
                for child in children {
                    child.collect_paths(&next, output);
                }
            }
        }
    }
}

/// Longest-prefix route decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteDecision {
    /// Forward to the child at the given index.
    Child(usize),
    /// Deliver locally.
    Local,
    /// Forward upward toward the parent.
    Parent,
    /// Silently drop.
    Drop,
}

/// Returns `true` if `prefix` is a path prefix of `path`.
pub fn is_prefix(prefix: &[String], path: &[String]) -> bool {
    prefix.len() <= path.len()
        && prefix
            .iter()
            .zip(path.iter())
            .all(|(left, right)| left == right)
}

/// Trait for resolving a destination path to a routing decision.
pub trait RouteProvider {
    /// Computes the routing decision for a destination path.
    fn route_destination(
        &self,
        local_path: &[String],
        child_paths: &[Vec<String>],
        has_parent: bool,
        dst_path: &[String],
    ) -> RouteDecision;
}

/// Default routing implementation using the protocol's longest-prefix rule.
pub struct DefaultRouteProvider;

impl RouteProvider for DefaultRouteProvider {
    fn route_destination(
        &self,
        local_path: &[String],
        child_paths: &[Vec<String>],
        has_parent: bool,
        dst_path: &[String],
    ) -> RouteDecision {
        let child = child_paths
            .iter()
            .enumerate()
            .filter(|(_, child_path)| is_prefix(child_path, dst_path))
            .max_by_key(|(_, child_path)| child_path.len())
            .map(|(index, _)| index);

        if let Some(index) = child {
            return RouteDecision::Child(index);
        }
        if local_path == dst_path {
            return RouteDecision::Local;
        }
        if has_parent && !is_prefix(local_path, dst_path) {
            return RouteDecision::Parent;
        }
        RouteDecision::Drop
    }
}

pub fn route_destination(
    local_path: &[String],
    child_paths: &[Vec<String>],
    has_parent: bool,
    dst_path: &[String],
) -> RouteDecision {
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
                &children,
                false,
                &[String::from("a"), String::from("b"), String::from("c")]
            ),
            RouteDecision::Child(1)
        );
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
