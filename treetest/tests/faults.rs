use treetest::{model::NodeId, scenarios::built_in_scenarios, sim::Simulation};
use unshell::protocol::ProtocolFault;

#[test]
fn unknown_leaf_and_unknown_procedure_fault_to_root() {
    let scenarios = built_in_scenarios();
    let mut simulation = Simulation::new(scenarios[4].clone()).expect("scenario should build");

    simulation
        .call_unchecked(
            NodeId(1),
            Some("missing_leaf"),
            "demo.leaf.v1.echo.invoke",
            Vec::new(),
        )
        .expect("unknown leaf call should start");
    simulation.drain().expect("network should drain");
    assert_eq!(
        simulation
            .latest_root_fault()
            .expect("root should observe unknown-leaf fault")
            .fault,
        ProtocolFault::UNKNOWN_LEAF
    );

    simulation
        .call_unchecked(
            NodeId(1),
            None,
            "demo.endpoint.v1.control.missing",
            Vec::new(),
        )
        .expect("unknown procedure call should start");
    simulation.drain().expect("network should drain");
    assert_eq!(
        simulation
            .latest_root_fault()
            .expect("root should observe unknown-procedure fault")
            .fault,
        ProtocolFault::UNKNOWN_PROCEDURE
    );
}

#[test]
fn invalid_hook_peer_faults_an_active_chat_hook() {
    let scenarios = built_in_scenarios();
    let mut simulation = Simulation::new(scenarios[5].clone()).expect("scenario should build");

    simulation
        .call_endpoint_procedure(NodeId(3), "demo.endpoint.v1.chat.session", b"open".to_vec())
        .expect("chat call should start");
    simulation.drain().expect("network should drain");
    let hook_id = *simulation.hook_ids().last().expect("hook should exist");

    simulation
        .inject_invalid_peer_data(
            NodeId(1),
            NodeId(0),
            hook_id,
            "demo.endpoint.v1.chat.session",
            "spoof",
        )
        .expect("invalid peer injection should enqueue");
    simulation.drain().expect("network should drain");

    assert_eq!(
        simulation
            .latest_root_fault()
            .expect("root should observe a fault")
            .fault,
        ProtocolFault::INVALID_HOOK_PEER
    );
    assert!(
        simulation
            .hooks
            .get(&hook_id)
            .expect("hook snapshot should exist")
            .closed
    );
}
