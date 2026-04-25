//! Path routing helpers and explicit enum tree declarations.

use alloc::{collections::BTreeMap, string::String, vec, vec::Vec};

/// Explicit test tree declaration used for configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeNode {
    Root { children: Vec<Self> },
    Endpoint {
        segment: String,
        leaves: Vec<LeafNode>,
        children: Vec<Self>,
    },
}

/// Leaf declaration used inside the explicit tree enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeafNode {
    pub name: String,
    pub procedures: Vec<String>,
}

impl TreeNode {
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
    Child(usize),
    Local,
    Parent,
    Drop,
}

/// One compiled routing table for one endpoint boundary.
#[derive(Debug, Clone, Default)]
pub struct CompiledRoutes {
    local_path: Vec<String>,
    has_parent: bool,
    nodes: Vec<RouteTrieNode>,
}

#[derive(Debug, Clone, Default)]
struct RouteTrieNode {
    best_child: Option<usize>,
    edges: BTreeMap<String, usize>,
}

impl CompiledRoutes {
    #[must_use]
    pub fn new(local_path: &[String], child_paths: &[Vec<String>], has_parent: bool) -> Self {
        let mut table = Self {
            local_path: local_path.to_vec(),
            has_parent,
            nodes: vec![RouteTrieNode::default()],
        };

        for (index, child_path) in child_paths.iter().enumerate() {
            table.insert_child(index, child_path);
        }

        table
    }

    fn insert_child(&mut self, index: usize, child_path: &[String]) {
        if !is_prefix(&self.local_path, child_path) || child_path.len() <= self.local_path.len() {
            return;
        }

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
pub fn is_prefix(prefix: &[String], path: &[String]) -> bool {
    prefix.len() <= path.len()
        && prefix
            .iter()
            .zip(path.iter())
            .all(|(left, right)| left == right)
}

/// Trait for resolving a destination path to a routing decision.
pub trait RouteProvider {
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
