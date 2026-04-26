//! Packet builders and endpoint construction.

use alloc::{collections::BTreeSet, string::String, vec::Vec};

use crate::protocol::tree::{HookKey, PendingHook};
use crate::protocol::{
    CallMessage, DataMessage, FrameBytes, HookTarget, PacketHeader, PacketType, ValidationError,
    encode_packet, validate_call, validate_header, validate_procedure_id,
};

use super::super::{CompiledRoutes, RouteDecision};
use super::core::{ChildRoute, EndpointError, EndpointOutcome, ProtocolEndpoint};
use crate::protocol::tree::LeafSpec;

impl ProtocolEndpoint {
    fn prepare_call(
        &self,
        dst_path: Vec<String>,
        dst_leaf: Option<String>,
        procedure_id: impl Into<String>,
        response_hook_id: Option<u64>,
        data: Vec<u8>,
    ) -> Result<(PacketHeader, CallMessage), EndpointError> {
        let procedure_id = procedure_id.into();
        validate_procedure_id(&procedure_id)?;

        let response_hook = response_hook_id.map(|hook_id| HookTarget {
            hook_id,
            return_path: self.path.clone(),
        });
        let header = PacketHeader {
            packet_type: PacketType::Call,
            src_path: self.path.clone(),
            dst_path,
            dst_leaf,
            hook_id: None,
        };
        let call = CallMessage {
            procedure_id,
            data,
            response_hook,
        };

        validate_header(&header)?;
        validate_call(&header, &call)?;
        Ok((header, call))
    }

    fn prepare_data(
        &self,
        dst_path: Vec<String>,
        hook_id: u64,
        procedure_id: impl Into<String>,
        data: Vec<u8>,
        end_hook: bool,
    ) -> Result<(PacketHeader, DataMessage), EndpointError> {
        let procedure_id = procedure_id.into();
        validate_procedure_id(&procedure_id)?;

        let header = PacketHeader {
            packet_type: PacketType::Data,
            src_path: self.path.clone(),
            dst_path,
            dst_leaf: None,
            hook_id: Some(hook_id),
        };
        let message = DataMessage {
            procedure_id,
            data,
            end_hook,
        };

        validate_header(&header)?;
        Ok((header, message))
    }

    fn register_outbound_call_hook(
        &mut self,
        header: &PacketHeader,
        call: &CallMessage,
    ) -> Result<(), EndpointError> {
        // Outbound calls reserve their response hook before the frame is emitted so
        // the endpoint can attribute returned Fault packets even before the callee
        // accepts the call. The hook only becomes active once valid hook traffic
        // comes back from the expected peer.
        if let Some(hook) = &call.response_hook
            && let key = HookKey::new(hook.return_path.clone(), hook.hook_id)
            && self
                .hooks
                .insert_pending(
                    key,
                    PendingHook {
                        caller_src_path: header.dst_path.clone(),
                        procedure_id: call.procedure_id.clone(),
                        local_ended: false,
                    },
                )
                .is_err()
        {
            return Err(EndpointError::Validation(ValidationError::InvalidHookId));
        }
        Ok(())
    }

    #[must_use]
    /// Creates an endpoint with compiled routing tables for its current topology.
    ///
    /// `parent_path` is currently used only as a presence flag. The endpoint stores its own
    /// absolute `path`, and routing only needs to know whether an upward route exists.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::{ChildRoute, LeafSpec, ProtocolEndpoint};
    /// let endpoint = ProtocolEndpoint::new(
    ///     vec!["worker".into()],
    ///     Some(Vec::new()),
    ///     vec![ChildRoute::registered(vec!["worker".into(), "child".into()])],
    ///     vec![LeafSpec {
    ///         name: "service".into(),
    ///         procedures: vec!["example.service.v1.invoke".into()],
    ///     }],
    /// );
    /// let _ = endpoint;
    /// ```
    pub fn new(
        path: Vec<String>,
        parent_path: Option<Vec<String>>,
        children: Vec<ChildRoute>,
        leaves: Vec<LeafSpec>,
    ) -> Self {
        let has_parent = parent_path.is_some();
        let registered_child_paths = children
            .iter()
            .filter(|child| child.registered)
            .map(|child| child.path.clone())
            .collect::<Vec<_>>();

        Self {
            local_id: None,
            parent_path,
            routing: CompiledRoutes::new(&path, &registered_child_paths, has_parent),
            path,
            children,
            leaves: leaves
                .into_iter()
                .map(|leaf| (leaf.name.clone(), leaf))
                .collect(),
            endpoint_procedures: BTreeSet::new(),
            hooks: Default::default(),
        }
    }

    #[must_use]
    /// Creates a root-assumed endpoint with one local identifier and predeclared leaves.
    ///
    /// What it is: a convenience constructor for the common bootstrap state where an endpoint has
    /// one local name but has not yet been assigned a non-root path by a parent connection.
    ///
    /// Why it exists: endpoint creation should not require every caller to manually pass an empty
    /// path, no parent, and no children just to host one or more known leaves.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::{LeafSpec, ProtocolEndpoint};
    /// let endpoint = ProtocolEndpoint::root(
    ///     "worker",
    ///     vec![LeafSpec {
    ///         name: "service".into(),
    ///         procedures: vec!["example.service.v1.invoke".into()],
    ///     }],
    /// );
    /// assert!(endpoint.path().is_empty());
    /// assert_eq!(endpoint.local_id(), Some("worker"));
    /// ```
    pub fn root(local_id: impl Into<String>, leaves: Vec<LeafSpec>) -> Self {
        let mut endpoint = Self::new(Vec::new(), None, Vec::new(), leaves);
        endpoint.local_id = Some(local_id.into());
        endpoint
    }

    #[must_use]
    /// Returns the endpoint's local bootstrap identifier, if one was assigned.
    ///
    /// What it is: a lightweight label separate from the protocol path.
    ///
    /// Why it exists: a freshly created endpoint may know its own local identity before a parent
    /// connection assigns its final tree path.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::ProtocolEndpoint;
    /// let endpoint = ProtocolEndpoint::root("worker", Vec::new());
    /// assert_eq!(endpoint.local_id(), Some("worker"));
    /// ```
    pub fn local_id(&self) -> Option<&str> {
        self.local_id.as_deref()
    }

    /// Returns the absolute path of this endpoint's direct parent, if one exists.
    ///
    /// What it is: the currently configured one-hop parent boundary for this
    /// endpoint.
    ///
    /// Why it exists: router-style leaves need to expose and inspect the tree edge
    /// they use for upstream traffic.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::ProtocolEndpoint;
    /// let endpoint = ProtocolEndpoint::new(vec!["worker".into()], Some(Vec::new()), Vec::new(), Vec::new());
    /// assert_eq!(endpoint.parent_path(), Some([].as_slice()));
    /// ```
    pub fn parent_path(&self) -> Option<&[String]> {
        self.parent_path.as_deref()
    }

    /// Returns the direct child routes currently known to this endpoint.
    ///
    /// What it is: the local routing-table inputs for direct descendants.
    ///
    /// Why it exists: management leaves often need to inspect or mirror the child
    /// topology they are controlling.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::{ChildRoute, ProtocolEndpoint};
    /// let endpoint = ProtocolEndpoint::new(
    ///     vec!["root".into()],
    ///     None,
    ///     vec![ChildRoute::registered(vec!["root".into(), "child".into()])],
    ///     Vec::new(),
    /// );
    /// assert_eq!(endpoint.child_routes().len(), 1);
    /// ```
    pub fn child_routes(&self) -> &[ChildRoute] {
        &self.children
    }

    /// Replaces the configured direct parent path and recompiles local routing.
    ///
    /// What it is: the supported way to attach or detach this endpoint from its
    /// upstream boundary.
    ///
    /// Why it exists: a router leaf should be able to promote or remove its parent
    /// connection without rebuilding the entire endpoint.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::ProtocolEndpoint;
    /// let mut endpoint = ProtocolEndpoint::new(vec!["root".into(), "worker".into()], Some(vec!["root".into()]), Vec::new(), Vec::new());
    /// endpoint.set_parent_path(None)?;
    /// assert!(endpoint.parent_path().is_none());
    /// # Ok::<(), unshell::protocol::tree::EndpointError>(())
    /// ```
    pub fn set_parent_path(
        &mut self,
        parent_path: Option<Vec<String>>,
    ) -> Result<(), EndpointError> {
        if let Some(path) = parent_path.as_deref() {
            self.validate_direct_parent_path(path)?;
        }
        self.parent_path = parent_path;
        self.rebuild_routing();
        Ok(())
    }

    /// Inserts or updates one direct child route and recompiles local routing.
    ///
    /// What it is: the supported mutation API for the endpoint's child list.
    ///
    /// Why it exists: router-management leaves need one invariant-preserving way to
    /// reflect child connection changes into path routing.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::{ChildRoute, ProtocolEndpoint};
    /// let mut endpoint = ProtocolEndpoint::new(vec!["root".into()], None, Vec::new(), Vec::new());
    /// endpoint.upsert_child_route(ChildRoute::registered(vec!["root".into(), "child".into()]))?;
    /// assert_eq!(endpoint.child_routes().len(), 1);
    /// # Ok::<(), unshell::protocol::tree::EndpointError>(())
    /// ```
    pub fn upsert_child_route(&mut self, route: ChildRoute) -> Result<(), EndpointError> {
        self.validate_direct_child_path(&route.path)?;
        if let Some(existing) = self.children.iter_mut().find(|child| child.path == route.path) {
            *existing = route;
        } else {
            self.children.push(route);
        }
        self.rebuild_routing();
        Ok(())
    }

    /// Removes one direct child route by absolute path and recompiles local routing.
    ///
    /// What it is: the supported mutation API for pruning a direct descendant.
    ///
    /// Why it exists: connection-management leaves need to tear down routes without
    /// mutating the endpoint internals directly.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::{ChildRoute, ProtocolEndpoint};
    /// let mut endpoint = ProtocolEndpoint::new(
    ///     vec!["root".into()],
    ///     None,
    ///     vec![ChildRoute::registered(vec!["root".into(), "child".into()])],
    ///     Vec::new(),
    /// );
    /// assert!(endpoint.remove_child_route(&[String::from("root"), String::from("child")]));
    /// assert!(endpoint.child_routes().is_empty());
    /// ```
    pub fn remove_child_route(&mut self, path: &[String]) -> bool {
        let original_len = self.children.len();
        self.children.retain(|child| child.path != path);
        let removed = self.children.len() != original_len;
        if removed {
            self.rebuild_routing();
        }
        removed
    }

    /// Registers a procedure that is handled directly by the endpoint.
    ///
    /// Endpoint-level procedures exist for protocol services that are not attached to one leaf,
    /// such as built-in runtime behavior.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::ProtocolEndpoint;
    /// let mut endpoint = ProtocolEndpoint::new(Vec::new(), None, Vec::new(), Vec::new());
    /// endpoint.add_endpoint_procedure("example.endpoint.v1.health")?;
    /// # Ok::<(), unshell::protocol::tree::EndpointError>(())
    /// ```
    pub fn add_endpoint_procedure(
        &mut self,
        procedure_id: impl Into<String>,
    ) -> Result<(), EndpointError> {
        let procedure_id = procedure_id.into();
        validate_procedure_id(&procedure_id)?;
        self.endpoint_procedures.insert(procedure_id);
        Ok(())
    }

    #[must_use]
    /// Allocates a hook id scoped to this endpoint path.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::ProtocolEndpoint;
    /// let mut endpoint = ProtocolEndpoint::new(Vec::new(), None, Vec::new(), Vec::new());
    /// let hook_id = endpoint.allocate_hook_id();
    /// assert_ne!(hook_id, 0);
    /// ```
    pub fn allocate_hook_id(&mut self) -> u64 {
        self.hooks.allocate_hook_id(&self.path)
    }

    fn rebuild_routing(&mut self) {
        let registered_child_paths = self
            .children
            .iter()
            .filter(|child| child.registered)
            .map(|child| child.path.clone())
            .collect::<Vec<_>>();
        self.routing = CompiledRoutes::new(
            &self.path,
            &registered_child_paths,
            self.parent_path.is_some(),
        );
    }

    fn validate_direct_parent_path(&self, parent_path: &[String]) -> Result<(), EndpointError> {
        let Some((_, expected_parent)) = self.path.split_last() else {
            return Err(EndpointError::Validation(ValidationError::TopologyInvariant(
                "root endpoints cannot declare a parent path",
            )));
        };
        if parent_path != expected_parent {
            return Err(EndpointError::Validation(ValidationError::TopologyInvariant(
                "parent path must equal the direct path prefix of this endpoint",
            )));
        }
        Ok(())
    }

    fn validate_direct_child_path(&self, child_path: &[String]) -> Result<(), EndpointError> {
        if child_path.len() != self.path.len() + 1 || !child_path.starts_with(&self.path) {
            return Err(EndpointError::Validation(ValidationError::TopologyInvariant(
                "child path must be one direct descendant of this endpoint",
            )));
        }
        Ok(())
    }

    /// Encodes a call frame without routing it through the local endpoint.
    ///
    /// This exists for callers that want a fully encoded outbound frame while handling transport
    /// themselves.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::ProtocolEndpoint;
    /// let mut endpoint = ProtocolEndpoint::new(Vec::new(), None, Vec::new(), Vec::new());
    /// let frame = endpoint.make_call(
    ///     vec!["worker".into()],
    ///     Some("service".into()),
    ///     "example.service.v1.invoke",
    ///     None,
    ///     vec![1, 2, 3],
    /// )?;
    /// assert!(!frame.is_empty());
    /// # Ok::<(), unshell::protocol::tree::EndpointError>(())
    /// ```
    pub fn make_call(
        &mut self,
        dst_path: Vec<String>,
        dst_leaf: Option<String>,
        procedure_id: impl Into<String>,
        response_hook_id: Option<u64>,
        data: Vec<u8>,
    ) -> Result<FrameBytes, EndpointError> {
        let (header, call) =
            self.prepare_call(dst_path, dst_leaf, procedure_id, response_hook_id, data)?;
        self.register_outbound_call_hook(&header, &call)?;
        Ok(encode_packet(&header, &call)?)
    }

    /// Builds and immediately routes a call, producing either a forward or a local event.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::{ChildRoute, EndpointOutcome, ProtocolEndpoint};
    /// let mut endpoint = ProtocolEndpoint::new(
    ///     Vec::new(),
    ///     None,
    ///     vec![ChildRoute::registered(vec!["worker".into()])],
    ///     Vec::new(),
    /// );
    /// let outcome = endpoint.send_call(
    ///     vec!["worker".into()],
    ///     Some("service".into()),
    ///     "example.service.v1.invoke",
    ///     None,
    ///     vec![],
    /// )?;
    /// assert!(matches!(outcome, EndpointOutcome::Forward { .. } | EndpointOutcome::Dropped | EndpointOutcome::Local(_)));
    /// # Ok::<(), unshell::protocol::tree::EndpointError>(())
    /// ```
    pub fn send_call(
        &mut self,
        dst_path: Vec<String>,
        dst_leaf: Option<String>,
        procedure_id: impl Into<String>,
        response_hook_id: Option<u64>,
        data: Vec<u8>,
    ) -> Result<EndpointOutcome, EndpointError> {
        let (header, call) =
            self.prepare_call(dst_path, dst_leaf, procedure_id, response_hook_id, data)?;
        self.register_outbound_call_hook(&header, &call)?;

        match self.decide_route(&header.dst_path) {
            RouteDecision::Local => self.handle_local_call(header, call),
            RouteDecision::Drop => {
                self.rollback_pending_call_hook(&call);
                Ok(EndpointOutcome::Dropped)
            }
            route => Ok(EndpointOutcome::Forward {
                route,
                frame: encode_packet(&header, &call)?,
            }),
        }
    }

    /// Encodes a data frame without routing it through the local endpoint.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::ProtocolEndpoint;
    /// let endpoint = ProtocolEndpoint::new(Vec::new(), None, Vec::new(), Vec::new());
    /// let frame = endpoint.make_data(vec!["root".into()], 7, "example.service.v1.invoke", vec![1], false)?;
    /// assert!(!frame.is_empty());
    /// # Ok::<(), unshell::protocol::tree::EndpointError>(())
    /// ```
    pub fn make_data(
        &self,
        dst_path: Vec<String>,
        hook_id: u64,
        procedure_id: impl Into<String>,
        data: Vec<u8>,
        end_hook: bool,
    ) -> Result<FrameBytes, EndpointError> {
        let (header, message) =
            self.prepare_data(dst_path, hook_id, procedure_id, data, end_hook)?;
        Ok(encode_packet(&header, &message)?)
    }

    /// Builds and immediately routes a data packet, updating local hook state for end-of-stream.
    ///
    /// # Example
    /// ```rust
    /// use unshell::protocol::tree::ProtocolEndpoint;
    /// let mut endpoint = ProtocolEndpoint::new(Vec::new(), None, Vec::new(), Vec::new());
    /// let _ = endpoint.send_data(vec!["root".into()], 7, "example.service.v1.invoke", vec![], false);
    /// # Ok::<(), unshell::protocol::tree::EndpointError>(())
    /// ```
    pub fn send_data(
        &mut self,
        dst_path: Vec<String>,
        hook_id: u64,
        procedure_id: impl Into<String>,
        data: Vec<u8>,
        end_hook: bool,
    ) -> Result<EndpointOutcome, EndpointError> {
        if let Some(active_key) = self
            .hooks
            .resolve_active_key(&dst_path, hook_id, &self.path)
            && self
                .hooks
                .active(&active_key)
                .is_some_and(|active| active.local_ended)
        {
            return Err(EndpointError::Validation(ValidationError::HookInvariant(
                "local side already closed this hook",
            )));
        }

        let local_end_dst_path = dst_path.clone();
        let host_key = HookKey::new(self.path.clone(), hook_id);
        let (header, message) =
            self.prepare_data(dst_path, hook_id, procedure_id, data, end_hook)?;

        if end_hook {
            self.mark_local_stream_end(&local_end_dst_path, hook_id, &host_key);
        }

        match self.decide_route(&header.dst_path) {
            RouteDecision::Local => self.handle_local_data(header, message),
            RouteDecision::Drop => Ok(EndpointOutcome::Dropped),
            route => Ok(EndpointOutcome::Forward {
                route,
                frame: encode_packet(&header, &message)?,
            }),
        }
    }

    fn rollback_pending_call_hook(&mut self, call: &CallMessage) {
        if let Some(hook) = &call.response_hook {
            self.hooks
                .remove_pending(&HookKey::new(hook.return_path.clone(), hook.hook_id));
        }
    }

    fn mark_local_stream_end(&mut self, dst_path: &[String], hook_id: u64, host_key: &HookKey) {
        // Locally-originated streams may not have been resolved against a peer yet, so fall
        // back to the endpoint's own hook key shape when closing them.
        let local_hook_key = self
            .hooks
            .resolve_active_key(dst_path, hook_id, &self.path)
            .unwrap_or_else(|| host_key.clone());
        if self.hooks.pending(host_key).is_some() {
            self.hooks.mark_pending_local_end(host_key);
        } else if self.hooks.mark_local_end(&local_hook_key) {
            self.hooks.remove_active(&local_hook_key);
        }
    }
}
