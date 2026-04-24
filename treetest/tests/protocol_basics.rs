use treetest::{scenarios::built_in_scenarios, sim::Simulation};

#[test]
fn endpoint_and_leaf_introspection_complete_successfully() {
    let scenarios = built_in_scenarios();
    let mut simulation = Simulation::new(scenarios[0].clone()).expect("scenario should build");

    simulation
        .call_endpoint_introspection(treetest::model::NodeId(1))
        .expect("endpoint introspection should start");
    simulation.drain().expect("network should drain");
    assert!(simulation.latest_root_data_text().is_some());

    simulation
        .call_leaf_introspection(treetest::model::NodeId(1), "echo")
        .expect("leaf introspection should start");
    simulation.drain().expect("network should drain");
    assert!(simulation.latest_root_data_text().is_some());
}

#[test]
fn echo_leaf_round_trips_user_payload() {
    let scenarios = built_in_scenarios();
    let mut simulation = Simulation::new(scenarios[1].clone()).expect("scenario should build");

    simulation
        .call_echo_leaf(treetest::model::NodeId(1), "echo", "hello from test")
        .expect("echo call should start");
    simulation.drain().expect("network should drain");

    assert_eq!(
        simulation.latest_root_data_text().as_deref(),
        Some("hello from test")
    );
    let hook_id = *simulation.hook_ids().last().expect("hook should exist");
    assert!(
        simulation
            .hooks
            .get(&hook_id)
            .expect("hook snapshot should exist")
            .closed
    );
}
