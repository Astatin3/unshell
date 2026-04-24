use treetest::{model::NodeId, scenarios::built_in_scenarios, sim::Simulation};

#[test]
fn bidirectional_chat_remains_active_until_bye() {
    let scenarios = built_in_scenarios();
    let mut simulation = Simulation::new(scenarios[3].clone()).expect("scenario should build");

    simulation
        .call_endpoint_procedure(NodeId(1), "demo.endpoint.v1.chat.session", b"open".to_vec())
        .expect("chat call should start");
    simulation.drain().expect("network should drain");

    let hook_id = *simulation.hook_ids().last().expect("hook should exist");
    assert!(
        !simulation
            .hooks
            .get(&hook_id)
            .expect("hook snapshot should exist")
            .closed
    );
    assert_eq!(
        simulation.latest_root_data_text().as_deref(),
        Some("chat ready")
    );

    simulation
        .send_root_hook_data(hook_id, "hello there", false)
        .expect("chat data should send");
    simulation.drain().expect("network should drain");
    assert_eq!(
        simulation.latest_root_data_text().as_deref(),
        Some("chat ack: HELLO THERE")
    );

    simulation
        .send_root_hook_data(hook_id, "bye", true)
        .expect("chat close should send");
    simulation.drain().expect("network should drain");
    assert!(
        simulation
            .hooks
            .get(&hook_id)
            .expect("hook snapshot should exist")
            .closed
    );
    assert_eq!(
        simulation.latest_root_data_text().as_deref(),
        Some("chat session closed")
    );
}
