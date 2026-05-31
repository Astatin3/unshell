use super::{
    constants::{
        BLOCK_BRAVO, BLOCK_CHARLIE, PROC_GET_BLOCK_STREAM, PROC_GET_CHILD_HASHES,
        PROC_GET_ROOT_HASH,
    },
    harness::MerkleHarness,
    tree::remote_fixture,
};

#[test]
fn merkle_sync_walks_hash_tree_and_streams_changed_blocks() {
    let mut harness = MerkleHarness::divergent();
    harness.assert_four_leaf_topology();

    let ticks = harness.run_until_done(100);
    assert!(
        ticks > 20,
        "sync should require many request/stream iterations"
    );

    let caller = harness.caller_report.borrow();
    assert_eq!(caller.final_root_hash, Some(harness.remote_root_hash));
    assert_eq!(caller.synchronized_blocks, [BLOCK_BRAVO, BLOCK_CHARLIE]);
    assert_eq!(
        caller.requested_procedures,
        [
            PROC_GET_ROOT_HASH,
            PROC_GET_CHILD_HASHES,
            PROC_GET_CHILD_HASHES,
            PROC_GET_CHILD_HASHES,
            PROC_GET_BLOCK_STREAM,
            PROC_GET_BLOCK_STREAM,
        ]
    );

    let respondent = harness.respondent_report.borrow();
    assert_eq!(respondent.requests_seen, caller.requested_procedures);
    assert_eq!(respondent.streams_started, 6);
    assert_eq!(respondent.streams_completed, 6);
    assert_eq!(respondent.frames_sent, 12);
    assert_eq!(harness.endpoint_b.hook_count(), 0);
}

#[test]
fn identical_tree_stops_after_root_hash() {
    let remote = remote_fixture();
    let mut harness = MerkleHarness::with_stores(remote.clone(), remote);

    harness.run_until_done(20);

    let caller = harness.caller_report.borrow();
    assert_eq!(caller.final_root_hash, Some(harness.remote_root_hash));
    assert_eq!(caller.requested_procedures, [PROC_GET_ROOT_HASH]);
    assert!(caller.synchronized_blocks.is_empty());

    let respondent = harness.respondent_report.borrow();
    assert_eq!(respondent.frames_sent, 1);
    assert_eq!(respondent.streams_started, 1);
    assert_eq!(respondent.streams_completed, 1);
}

#[test]
fn block_stream_hook_persists_until_final_frame() {
    let mut harness = MerkleHarness::divergent();

    harness.run_until_respondent_frames(8, 100);
    assert_eq!(
        harness.endpoint_b.hook_count(),
        1,
        "first block stream should keep its hook after a non-final chunk"
    );

    harness.run_until_done(100);
    assert!(
        harness.endpoint_b.hook_count() == 0,
        "final block stream packet should clean respondent hook state"
    );
}
