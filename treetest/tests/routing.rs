use treetest::{model::NodeId, scenarios::built_in_scenarios, sim::Simulation};

#[test]
fn nested_route_uses_longest_prefix_path() {
    let scenarios = built_in_scenarios();
    let mut simulation = Simulation::new(scenarios[2].clone()).expect("scenario should build");

    simulation
        .call_echo_leaf(NodeId(2), "echo", "nested")
        .expect("echo call should start");
    simulation.drain().expect("network should drain");

    let trace_text = simulation
        .trace
        .iter()
        .map(|event| event.summary.clone())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(trace_text.contains("forwarded frame to child /alpha"));
    assert!(trace_text.contains("forwarded frame to child /alpha/beta"));
    assert_eq!(
        simulation.latest_root_data_text().as_deref(),
        Some("nested")
    );
}

#[test]
fn chunked_endpoint_procedure_returns_multiple_packets() {
    let scenarios = built_in_scenarios();
    let mut simulation = Simulation::new(scenarios[1].clone()).expect("scenario should build");

    simulation
        .call_endpoint_procedure(
            NodeId(1),
            "demo.endpoint.v1.stream.chunked_greeting",
            b"go".to_vec(),
        )
        .expect("procedure call should start");
    simulation.drain().expect("network should drain");

    let root_data_count = simulation
        .recorded_events
        .iter()
        .filter(|event| matches!(event, treetest::sim::RecordedEvent::Data { node_path, .. } if node_path == "/"))
        .count();
    assert!(root_data_count >= 3);
    assert_eq!(
        simulation.latest_root_data_text().as_deref(),
        Some("chunk 3: hook complete")
    );
}
