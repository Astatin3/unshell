//! # Tree Module
//!
//! The `Tree` dispatches incoming [`TreeRequest`]s to registered [`Endpoint`]s
//! by matching the request's destination path.
//!
//! ## Path matching
//!
//! Paths are `/`-delimited strings. An `Endpoint` is registered at a path prefix.
//! A request matches an endpoint if the endpoint's path is a prefix of the request path.
//! When multiple endpoints match, the one with the **longest** prefix wins.
//!
//! ```text
//! Registered endpoints:           Request path:
//!   /shell      ← prefix          /shell/exec     → matches /shell
//!   /files      ← prefix          /files/read     → matches /files
//!   /shell/exec ← more specific   /shell/exec     → matches /shell/exec (longer)
//! ```
//!
//! ## Usage
//!
//! ```rust
//! use unshell::tree::{Tree, Endpoint};
//! use unshell::protocol::{
//!     TreeRequest, TreeResponse, RequestType, ResponseStatus, content,
//! };
//!
//! /// A simple echo endpoint that reflects the request data back.
//! struct EchoEndpoint;
//!
//! impl Endpoint for EchoEndpoint {
//!     fn handle(&mut self, request: TreeRequest) -> TreeResponse {
//!         TreeResponse {
//!             request_id: request.request_id,
//!             status: ResponseStatus::Ok,
//!             content_type: request.content_type.clone(),
//!             data: request.data.clone(),
//!         }
//!     }
//! }
//!
//! let mut tree = Tree::new();
//! tree.register("/echo", EchoEndpoint);
//!
//! let response = tree.dispatch(TreeRequest {
//!     request_id: 1,
//!     request_type: RequestType::Read,
//!     content_type: content::UTF8_STRING.into(),
//!     data: b"hello".to_vec(),
//! }, "/echo/anything");
//!
//! assert_eq!(response.status, ResponseStatus::Ok);
//! assert_eq!(response.data, b"hello");
//! ```

extern crate alloc;
use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use crate::protocol::{
    content, ResponseStatus, TreeRequest, TreeResponse,
};

// ---------------------------------------------------------------------------
// Endpoint trait
// ---------------------------------------------------------------------------

/// A module that handles [`TreeRequest`]s at a registered path.
///
/// Implement this trait to add capabilities to a payload. The `Tree` calls
/// `handle` when a request's path matches this endpoint's registration prefix.
///
/// # Example
///
/// ```rust
/// use unshell::tree::Endpoint;
/// use unshell::protocol::{TreeRequest, TreeResponse, ResponseStatus, content};
///
/// struct PingEndpoint;
///
/// impl Endpoint for PingEndpoint {
///     fn handle(&mut self, request: TreeRequest) -> TreeResponse {
///         TreeResponse {
///             request_id: request.request_id,
///             status: ResponseStatus::Ok,
///             content_type: content::UTF8_STRING.into(),
///             data: b"pong".to_vec(),
///         }
///     }
/// }
/// ```
pub trait Endpoint: Send {
    /// Handle a request and return a response.
    ///
    /// This method is called synchronously on the recv loop thread. It should
    /// not block for extended periods. For long-running operations, spawn a
    /// background thread and return immediately with a `pending` response
    /// (streaming responses are a future protocol feature).
    fn handle(&mut self, request: TreeRequest) -> TreeResponse;
}

// ---------------------------------------------------------------------------
// Tree
// ---------------------------------------------------------------------------

/// A path-addressed dispatcher that routes [`TreeRequest`]s to [`Endpoint`]s.
///
/// # Path matching algorithm
///
/// The tree uses **longest-prefix matching**:
/// 1. Split the request path by `/`.
/// 2. For each registered endpoint, check if the endpoint's path components
///    are a prefix of the request path components.
/// 3. Among all matching endpoints, return the one with the most components
///    (the most specific match).
/// 4. If no match: return a [`ResponseStatus::NoBranchError`] response.
///
/// # Example
///
/// ```rust
/// use unshell::tree::{Tree, Endpoint};
/// use unshell::protocol::{TreeRequest, TreeResponse, RequestType, ResponseStatus, content};
///
/// struct Shell;
///
/// impl Endpoint for Shell {
///     fn handle(&mut self, req: TreeRequest) -> TreeResponse {
///         TreeResponse {
///             request_id: req.request_id,
///             status: ResponseStatus::Ok,
///             content_type: content::UTF8_STRING.into(),
///             data: b"shell output".to_vec(),
///         }
///     }
/// }
///
/// let mut tree = Tree::new();
/// tree.register("/shell", Shell);
///
/// // A request to /shell/exec/anything matches /shell (the registered prefix).
/// let resp = tree.dispatch(
///     TreeRequest {
///         request_id: 1,
///         request_type: RequestType::CallProcedure,
///         content_type: content::NONE.into(),
///         data: Vec::new(),
///     },
///     "/shell/exec",
/// );
/// assert_eq!(resp.status, ResponseStatus::Ok);
/// ```
pub struct Tree {
    /// Registered endpoints with their path prefixes.
    ///
    /// The path is stored as a `Vec<String>` of components (split on `/`,
    /// empty leading component from the leading `/` is discarded).
    endpoints: Vec<(Vec<String>, Box<dyn Endpoint>)>,
}

impl Tree {
    /// Create an empty tree with no registered endpoints.
    #[must_use]
    pub fn new() -> Self {
        Self {
            endpoints: Vec::new(),
        }
    }

    /// Register an endpoint at the given path prefix.
    ///
    /// # Arguments
    ///
    /// * `path` — the path prefix this endpoint owns, e.g. `"/shell"`.
    ///   Leading `/` is stripped; components are split on `/`.
    /// * `endpoint` — the handler that will receive matching requests.
    ///
    /// # Panics
    ///
    /// Does not panic. Registering the same path twice is allowed; the second
    /// registration shadows the first for that exact path (longest-prefix
    /// matching still applies for sub-paths).
    ///
    /// # Example
    ///
    /// ```rust
    /// use unshell::tree::{Tree, Endpoint};
    /// use unshell::protocol::{TreeRequest, TreeResponse, ResponseStatus, content};
    ///
    /// struct Noop;
    /// impl Endpoint for Noop {
    ///     fn handle(&mut self, req: TreeRequest) -> TreeResponse {
    ///         TreeResponse {
    ///             request_id: req.request_id,
    ///             status: ResponseStatus::Ok,
    ///             content_type: content::NONE.into(),
    ///             data: Vec::new(),
    ///         }
    ///     }
    /// }
    ///
    /// let mut tree = Tree::new();
    /// tree.register("/shell", Noop);
    /// ```
    pub fn register<E: Endpoint + 'static>(&mut self, path: &str, endpoint: E) {
        let components = split_path(path);
        self.endpoints.push((components, Box::new(endpoint)));
    }

    /// Dispatch a request to the best-matching endpoint.
    ///
    /// Returns a [`TreeResponse`] with [`ResponseStatus::NoBranchError`]
    /// if no registered endpoint matches the request path.
    ///
    /// # Arguments
    ///
    /// * `request` — the incoming request.
    /// * `dst_path` — the destination path from the packet header.
    ///
    /// # Example
    ///
    /// ```rust
    /// use unshell::tree::Tree;
    /// use unshell::protocol::{TreeRequest, RequestType, ResponseStatus, content};
    ///
    /// let mut tree = Tree::new();
    /// // (register some endpoints here)
    ///
    /// let resp = tree.dispatch(
    ///     TreeRequest {
    ///         request_id: 99,
    ///         request_type: RequestType::Read,
    ///         content_type: content::NONE.into(),
    ///         data: Vec::new(),
    ///     },
    ///     "/unknown/path",
    /// );
    /// assert_eq!(resp.status, ResponseStatus::NoBranchError);
    /// ```
    pub fn dispatch(&mut self, request: TreeRequest, dst_path: &str) -> TreeResponse {
        let path_components = split_path(dst_path);

        // Find the endpoint with the longest matching prefix.
        let best = self
            .endpoints
            .iter_mut()
            .filter(|(ep_path, _)| is_prefix(ep_path, &path_components))
            .max_by_key(|(ep_path, _)| ep_path.len());

        match best {
            Some((_, endpoint)) => endpoint.handle(request),
            None => TreeResponse {
                request_id: request.request_id,
                status: ResponseStatus::NoBranchError,
                content_type: content::NONE.into(),
                data: Vec::new(),
            },
        }
    }

    /// Return the list of registered path prefixes.
    ///
    /// Used during handshake to tell the router which paths this tree owns.
    ///
    /// # Example
    ///
    /// ```rust
    /// use unshell::tree::{Tree, Endpoint};
    /// use unshell::protocol::{TreeRequest, TreeResponse, ResponseStatus, content};
    ///
    /// struct Noop;
    /// impl Endpoint for Noop {
    ///     fn handle(&mut self, req: TreeRequest) -> TreeResponse {
    ///         TreeResponse {
    ///             request_id: req.request_id,
    ///             status: ResponseStatus::Ok,
    ///             content_type: content::NONE.into(),
    ///             data: Vec::new(),
    ///         }
    ///     }
    /// }
    ///
    /// let mut tree = Tree::new();
    /// tree.register("/shell", Noop);
    /// tree.register("/files", Noop);
    ///
    /// let paths = tree.registered_paths("/agents/abc123");
    /// assert!(paths.contains(&"/agents/abc123/shell".to_string()));
    /// assert!(paths.contains(&"/agents/abc123/files".to_string()));
    /// ```
    #[must_use]
    pub fn registered_paths(&self, base_prefix: &str) -> Vec<String> {
        let base = base_prefix.trim_end_matches('/');
        self.endpoints
            .iter()
            .map(|(components, _)| {
                let sub = components.join("/");
                if sub.is_empty() {
                    base.to_owned()
                } else {
                    alloc::format!("{base}/{sub}")
                }
            })
            .collect()
    }
}

impl Default for Tree {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Path utilities
// ---------------------------------------------------------------------------

/// Split a path string into its components.
///
/// Leading `/` and empty segments are discarded.
///
/// ```text
/// "/shell/exec"  → ["shell", "exec"]
/// "/shell/"      → ["shell"]
/// "shell"        → ["shell"]
/// "/"            → []
/// ```
fn split_path(path: &str) -> Vec<String> {
    path.split('/')
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

/// Returns `true` if `prefix` is a prefix of (or equal to) `path`.
///
/// Both are slices of path components (already split on `/`).
///
/// ```text
/// prefix = ["shell"]           path = ["shell", "exec"]  → true
/// prefix = ["shell", "exec"]   path = ["shell", "exec"]  → true (exact match)
/// prefix = ["shell", "exec"]   path = ["shell"]          → false (prefix longer)
/// prefix = ["files"]           path = ["shell", "exec"]  → false (different root)
/// ```
fn is_prefix(prefix: &[String], path: &[String]) -> bool {
    if prefix.len() > path.len() {
        return false;
    }
    prefix.iter().zip(path.iter()).all(|(a, b)| a == b)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{RequestType, ResponseStatus, content};

    // A minimal endpoint that echoes the request data.
    struct Echo;
    impl Endpoint for Echo {
        fn handle(&mut self, req: TreeRequest) -> TreeResponse {
            TreeResponse {
                request_id: req.request_id,
                status: ResponseStatus::Ok,
                content_type: req.content_type,
                data: req.data,
            }
        }
    }

    // A minimal endpoint that always returns a fixed string.
    struct Fixed(&'static str);
    impl Endpoint for Fixed {
        fn handle(&mut self, req: TreeRequest) -> TreeResponse {
            TreeResponse {
                request_id: req.request_id,
                status: ResponseStatus::Ok,
                content_type: content::UTF8_STRING.into(),
                data: self.0.as_bytes().to_vec(),
            }
        }
    }

    fn make_req(id: u64) -> TreeRequest {
        TreeRequest {
            request_id: id,
            request_type: RequestType::Read,
            content_type: content::NONE.into(),
            data: Vec::new(),
        }
    }

    /// A single endpoint is matched correctly.
    #[test]
    fn single_endpoint_match() {
        let mut tree = Tree::new();
        tree.register("/shell", Echo);

        let resp = tree.dispatch(make_req(1), "/shell/exec");
        assert_eq!(resp.status, ResponseStatus::Ok, "expected Ok for /shell/exec");
        assert_eq!(resp.request_id, 1);
    }

    /// When two endpoints are registered, the second one is also reachable.
    ///
    /// This test specifically catches the old `return None` bug in `get_endpoint`:
    /// the first endpoint (/files) doesn't match /shell/exec, so the tree must
    /// continue to the second entry (/shell).
    #[test]
    fn second_endpoint_match() {
        let mut tree = Tree::new();
        tree.register("/files", Fixed("files"));
        tree.register("/shell", Fixed("shell"));

        let resp = tree.dispatch(make_req(2), "/shell/exec");
        assert_eq!(resp.status, ResponseStatus::Ok);
        assert_eq!(resp.data, b"shell");
    }

    /// No matching endpoint returns NoBranchError.
    #[test]
    fn no_match_returns_no_branch_error() {
        let mut tree = Tree::new();
        tree.register("/shell", Echo);

        let resp = tree.dispatch(make_req(3), "/nonexistent/path");
        assert_eq!(resp.status, ResponseStatus::NoBranchError);
        assert_eq!(resp.request_id, 3);
    }

    /// Longer (more specific) prefix wins over shorter prefix.
    #[test]
    fn longer_prefix_wins() {
        let mut tree = Tree::new();
        tree.register("/shell", Fixed("short"));
        tree.register("/shell/exec", Fixed("long"));

        let resp = tree.dispatch(make_req(4), "/shell/exec/anything");
        assert_eq!(resp.data, b"long", "longer prefix should win");
    }

    /// A request path that is shorter than the registered prefix does not match.
    #[test]
    fn prefix_does_not_overmatch() {
        let mut tree = Tree::new();
        tree.register("/shell/exec/something", Echo);

        // /shell/exec is shorter than the registered path — should NOT match
        let resp = tree.dispatch(make_req(5), "/shell/exec");
        assert_eq!(resp.status, ResponseStatus::NoBranchError);
    }

    /// `registered_paths` returns all prefixes with the base path prepended.
    #[test]
    fn registered_paths_prepends_base() {
        let mut tree = Tree::new();
        tree.register("/shell", Echo);
        tree.register("/files", Echo);

        let paths = tree.registered_paths("/agents/abc123");
        assert!(paths.contains(&"/agents/abc123/shell".to_string()));
        assert!(paths.contains(&"/agents/abc123/files".to_string()));
        assert_eq!(paths.len(), 2);
    }

    // -----------------------------------------------------------------------
    // Path utility tests
    // -----------------------------------------------------------------------

    #[test]
    fn split_path_leading_slash() {
        assert_eq!(split_path("/shell/exec"), vec!["shell", "exec"]);
    }

    #[test]
    fn split_path_no_leading_slash() {
        assert_eq!(split_path("shell/exec"), vec!["shell", "exec"]);
    }

    #[test]
    fn split_path_trailing_slash() {
        assert_eq!(split_path("/shell/"), vec!["shell"]);
    }

    #[test]
    fn split_path_root() {
        let result: Vec<String> = split_path("/");
        assert!(result.is_empty());
    }

    #[test]
    fn is_prefix_exact_match() {
        let p = split_path("/shell/exec");
        assert!(is_prefix(&p, &p));
    }

    #[test]
    fn is_prefix_valid() {
        let prefix = split_path("/shell");
        let path = split_path("/shell/exec");
        assert!(is_prefix(&prefix, &path));
    }

    #[test]
    fn is_prefix_prefix_too_long() {
        let prefix = split_path("/shell/exec");
        let path = split_path("/shell");
        assert!(!is_prefix(&prefix, &path));
    }

    #[test]
    fn is_prefix_different_root() {
        let prefix = split_path("/files");
        let path = split_path("/shell/exec");
        assert!(!is_prefix(&prefix, &path));
    }
}
