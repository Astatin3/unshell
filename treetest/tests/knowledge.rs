use treetest::{model::NodeId, scenarios::built_in_scenarios, sim::Simulation};

#[test]
fn realistic_mode_memory_reset_forgets_deeper_nodes() {
    let scenarios = built_in_scenarios();
    let mut simulation = Simulation::new(scenarios[2].clone()).expect("scenario should build");

    simulation
        .call_echo_leaf(NodeId(2), "echo", "learn this nested leaf")
        .expect("echo call should start");
    simulation.drain().expect("network should drain");

    assert!(
        simulation
            .root_knowledge
            .node(&simulation.node(NodeId(2)).path)
            .is_some()
    );

    simulation.enable_realistic_mode_with_memory_reset();

    assert!(simulation.is_realistic_mode());
    assert!(
        simulation
            .root_knowledge
            .node(&simulation.node(NodeId(2)).path)
            .is_none()
    );
    assert!(
        simulation
            .root_knowledge
            .node(&simulation.node(NodeId(1)).path)
            .is_some()
    );
    assert!(
        simulation
            .root_knowledge
            .node(&simulation.node(NodeId(0)).path)
            .is_some()
    );
}
