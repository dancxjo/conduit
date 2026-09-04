use super::*;

#[test]
fn mechanism_free_form_plans_two_exact_directional_lines() {
    for forbidden in [
        "forebrain",
        "motherbrain",
        "host",
        "boot",
        "websocket",
        "wifi",
        "address",
        "socket",
        "authority",
    ] {
        assert!(!BODY_COORDINATION_SOURCE
            .to_ascii_lowercase()
            .contains(forbidden));
    }
    let exact = exact_body_coordination_plan(
        BootId::from("forebrain/boot-1"),
        BootId::from("motherbrain/boot-1"),
        "wifi/interbrain-1",
    )
    .unwrap();
    assert!(conduit_core::verify_plan(&exact.plan));
    assert_eq!(exact.plan.fragments.len(), 2);
    let connections = exact
        .plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .collect::<Vec<_>>();
    assert_eq!(connections.len(), 4);
    let remote = connections
        .iter()
        .filter(|connection| connection.selected_line.is_some())
        .collect::<Vec<_>>();
    assert_eq!(remote.len(), 4);
    let remote_ids = remote
        .iter()
        .map(|connection| &connection.connection_id)
        .collect::<alloc::collections::BTreeSet<_>>();
    assert_eq!(remote_ids.len(), 2);
    assert_ne!(exact.outbound_line.line_id, exact.return_line.line_id);
    assert_eq!(
        exact.outbound_line.binding.base_instance_id,
        exact.return_line.binding.base_instance_id
    );
    assert_eq!(
        exact.outbound_line.binding.source.host_id,
        exact.forebrain.host_id
    );
    assert_eq!(
        exact.return_line.binding.source.host_id,
        exact.motherbrain.host_id
    );
    assert_eq!(exact.outbound_line.contract.scope, LineScope::LocalNetwork);
    assert_eq!(
        exact.outbound_line.contract.duplex,
        conduit_core::LineDuplex::FullDuplex
    );
    assert_eq!(
        exact.outbound_line.contract.security,
        LineSecurity::PlaintextNetwork
    );
    assert!(remote
        .iter()
        .all(|connection| connection.admitted_lines.len() == 1));
}

#[test]
fn stale_boot_cannot_reuse_the_exact_line_offer() {
    let exact = exact_body_coordination_plan(
        BootId::from("forebrain/boot-1"),
        BootId::from("motherbrain/boot-1"),
        "wifi/interbrain-1",
    )
    .unwrap();
    let mut stale = exact.outbound_line;
    stale.binding.sink.boot_id = BootId::from("motherbrain/boot-stale");
    let planned = exact
        .plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .find_map(|connection| connection.selected_line.as_ref())
        .unwrap();
    assert_ne!(stale.binding.bound_link(), planned.binding);
}

#[test]
fn selected_line_loss_requires_ordinary_replanning_without_mutating_plan() {
    let loss = exact_body_coordination_line_loss(
        BootId::from("forebrain/boot-1"),
        BootId::from("motherbrain/boot-1"),
        "wifi/interbrain-1",
        FOREBRAIN_TO_MOTHERBRAIN_LINE,
    )
    .unwrap();
    assert!(loss.replan_required);
    assert_eq!(
        loss.unavailable_line_id.as_str(),
        FOREBRAIN_TO_MOTHERBRAIN_LINE
    );
    assert!(loss.refusal.contains("unavailable"));
    let accepted = exact_body_coordination_plan(
        BootId::from("forebrain/boot-1"),
        BootId::from("motherbrain/boot-1"),
        "wifi/interbrain-1",
    )
    .unwrap();
    assert_eq!(loss.plan_id, accepted.plan.plan_id);
}
