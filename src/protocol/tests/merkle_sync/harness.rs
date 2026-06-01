use alloc::{rc::Rc, vec};
use core::cell::RefCell;

use crate::protocol::{Endpoint, Leaf};

use super::{
    constants::{
        ENDPOINT_CALLER, ENDPOINT_RESPONDENT, LEAF_MERKLE_CALLER, LEAF_MERKLE_RESPONDENT,
        LEAF_MOCK_CONNECTION,
    },
    leaves::{MerkleCallerLeaf, MerkleRespondentLeaf, MockConnectionLeaf},
    state::{CallerReport, RespondentReport},
    tree::{MerkleStore, local_fixture, remote_fixture},
};

/// Complete two-endpoint Merkle sync test harness.
///
/// Endpoint A owns the caller leaf and one mock connection leaf. Endpoint B owns the
/// respondent leaf and the opposite mock connection leaf. Reports are shared out of
/// the boxed leaf objects so tests can assert state without downcasting trait
/// objects.
pub(super) struct MerkleHarness {
    pub(super) endpoint_a: Endpoint,
    pub(super) endpoint_b: Endpoint,
    caller_leaf: MerkleCallerLeaf,
    caller_connection: MockConnectionLeaf,
    respondent_leaf: MerkleRespondentLeaf,
    respondent_connection: MockConnectionLeaf,
    pub(super) caller_report: Rc<RefCell<CallerReport>>,
    pub(super) respondent_report: Rc<RefCell<RespondentReport>>,
    pub(super) remote_root_hash: u32,
}

impl MerkleHarness {
    /// Creates the divergent fixture used by the main sync test.
    pub(super) fn divergent() -> Self {
        Self::with_stores(local_fixture(), remote_fixture())
    }

    /// Creates a custom caller/respondent fixture.
    pub(super) fn with_stores(local: MerkleStore, remote: MerkleStore) -> Self {
        let remote_root_hash = remote.root_hash();
        let caller_report = Rc::new(RefCell::new(CallerReport::default()));
        let respondent_report = Rc::new(RefCell::new(RespondentReport::default()));
        let (tx_a, rx_a) = crossbeam_channel::unbounded();
        let (tx_b, rx_b) = crossbeam_channel::unbounded();

        let mut endpoint_a = Endpoint::new(ENDPOINT_CALLER);
        endpoint_a.path = vec![ENDPOINT_CALLER];

        let mut endpoint_b = Endpoint::new(ENDPOINT_RESPONDENT);
        endpoint_b.path = vec![ENDPOINT_CALLER, ENDPOINT_RESPONDENT];

        // Register routes before the first caller update so initial packet delivery
        // does not depend on leaf ordering.
        endpoint_a.add_connection(ENDPOINT_RESPONDENT, false);
        endpoint_b.add_connection(ENDPOINT_CALLER, true);

        Self {
            endpoint_a,
            endpoint_b,
            caller_leaf: MerkleCallerLeaf::new(local, caller_report.clone()),
            caller_connection: MockConnectionLeaf::new(tx_b, rx_a, ENDPOINT_RESPONDENT, false),
            respondent_leaf: MerkleRespondentLeaf::new(remote, respondent_report.clone()),
            respondent_connection: MockConnectionLeaf::new(tx_a, rx_b, ENDPOINT_CALLER, true),
            caller_report,
            respondent_report,
            remote_root_hash,
        }
    }

    /// Drives one deterministic protocol loop.
    pub(super) fn tick(&mut self) {
        self.caller_leaf.update(&mut self.endpoint_a);
        self.caller_connection.update(&mut self.endpoint_a);
        self.respondent_leaf.update(&mut self.endpoint_b);
        self.respondent_connection.update(&mut self.endpoint_b);
    }

    /// Runs until the caller reports completion.
    pub(super) fn run_until_done(&mut self, max_ticks: usize) -> usize {
        for tick in 1..=max_ticks {
            self.tick();

            if self.caller_report.borrow().done {
                return tick;
            }
        }

        panic!("Merkle sync did not finish within {max_ticks} ticks");
    }

    /// Runs until the respondent has sent at least `target_frames` frames.
    pub(super) fn run_until_respondent_frames(
        &mut self,
        target_frames: usize,
        max_ticks: usize,
    ) -> usize {
        for tick in 1..=max_ticks {
            self.tick();

            if self.respondent_report.borrow().frames_sent >= target_frames {
                return tick;
            }
        }

        panic!("respondent did not send {target_frames} frames within {max_ticks} ticks");
    }

    /// Verifies the requested four-leaf topology.
    pub(super) fn assert_four_leaf_topology(&self) {
        assert_eq!(self.caller_leaf.get_id(), LEAF_MERKLE_CALLER);
        assert_eq!(self.caller_connection.get_id(), LEAF_MOCK_CONNECTION);
        assert_eq!(self.respondent_leaf.get_id(), LEAF_MERKLE_RESPONDENT);
        assert_eq!(self.respondent_connection.get_id(), LEAF_MOCK_CONNECTION);
    }
}
