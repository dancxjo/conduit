use crate::{
    form_source,
    product_execution::{ProductExecutionContext, ProductRuntime},
    two_std_line,
};
use conduit_core::{BootId, ConnectionBase, GearId, HostId, OfferGeneration};
use conduit_planner::{PlacementChoice, PlacementChoices};
use conduit_std_host::{StdHost, StdHostConfig};
use conduit_wire::{SessionBinding, SessionMachine, SessionRole};
use std::{collections::BTreeMap, path::PathBuf};

fn form() -> conduit_form::ExpandedCanonicalForm {
    form_source::load_signal(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/signal-demo.conduit"),
    )
    .unwrap()
    .expand_entry()
    .unwrap()
}

fn planned() -> (ProductExecutionContext, conduit_core::Plan) {
    let context = two_std_line::context().unwrap();
    let plan = context
        .plan_with_placements(&form(), &two_std_line::placements(&form()).unwrap())
        .unwrap();
    (context, plan)
}

fn binding(plan: &conduit_core::Plan) -> SessionBinding {
    let source = plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == two_std_line::SOURCE_HOST)
        .unwrap();
    let sink = plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == two_std_line::SINK_HOST)
        .unwrap();
    SessionBinding::from_planned_connection(
        plan.plan_id.clone(),
        source.fragment_id.clone(),
        sink.fragment_id.clone(),
        &source.connections[0],
    )
    .unwrap()
}

#[test]
fn installed_fixture_runs_two_std_kernels_over_exact_line() {
    let source = include_str!("../../../examples/signal-demo.conduit").to_ascii_lowercase();
    for forbidden in [
        "host",
        "boot",
        "websocket",
        "address",
        "process",
        "transport",
    ] {
        assert!(!source.contains(forbidden), "Form contains {forbidden}");
    }
    let (_, plan) = planned();
    let evidence = two_std_line::execute(&plan).unwrap();
    assert_eq!(evidence.received, 16);
    assert_eq!(evidence.pressure_retries, 1);
    assert_eq!(plan.fragments.len(), 2);
    assert!(plan.fragments.iter().all(|fragment| fragment.connections[0]
        .selected_line
        .as_ref()
        .unwrap()
        .line_id
        .as_str()
        == "product/two-std/websocket-line"));
}

#[test]
fn wrong_boot_and_missing_peer_truth_refuse_planning() {
    let source = two_std_line::host(two_std_line::SOURCE_HOST);
    let sink = two_std_line::host(two_std_line::SINK_HOST);
    let mut stale = two_std_line::line_offer(&source, &sink);
    stale.binding.sink.boot_id = BootId::from("stale-boot");
    let context = ProductExecutionContext::new(
        vec![source.advertisement().clone(), sink.advertisement().clone()],
        vec![ProductRuntime::std(source), ProductRuntime::std(sink)],
        vec![ConnectionBase::WebSocket],
        vec![stale],
    )
    .unwrap();
    assert!(context
        .plan_with_placements(&form(), &two_std_line::placements(&form()).unwrap())
        .is_err());

    let source = two_std_line::host(two_std_line::SOURCE_HOST);
    let sink = two_std_line::host(two_std_line::SINK_HOST);
    let absent = ProductExecutionContext::new(
        vec![source.advertisement().clone(), sink.advertisement().clone()],
        vec![ProductRuntime::std(source), ProductRuntime::std(sink)],
        vec![ConnectionBase::WebSocket],
        Vec::new(),
    )
    .unwrap();
    assert!(absent
        .plan_with_placements(&form(), &two_std_line::placements(&form()).unwrap())
        .is_err());
}

#[test]
fn stale_session_and_duplicate_frame_are_machine_readable_refusals() {
    let (_, plan) = planned();
    let exact = binding(&plan);
    let mut sink = SessionMachine::new(exact.clone(), SessionRole::Sink).unwrap();
    sink.admit_inbound(exact.hello_frame()).unwrap();
    assert_eq!(
        sink.admit_inbound(exact.hello_frame()),
        Err(conduit_wire::WireError::DuplicateFrame)
    );

    let mut stale = exact.clone();
    stale.plan_id = conduit_core::PlanId::from("stale-plan-session");
    let mut exact_sink = SessionMachine::new(exact.clone(), SessionRole::Sink).unwrap();
    assert_eq!(
        exact_sink.admit_inbound(stale.hello_frame()),
        Err(conduit_wire::WireError::PlanMismatch)
    );
}

#[test]
fn absent_runtime_peer_reaches_bounded_transport_refusal() {
    let listener =
        conduit_std_host::websocket::NativeWebSocketListener::bind_loopback(2048).unwrap();
    assert!(matches!(
        listener.accept_with_timeout(std::time::Duration::from_millis(1)),
        Err(conduit_std_host::websocket::NativeWebSocketError::AcceptDeadline)
    ));
}

#[test]
fn old_plan_refuses_after_current_host_truth_changes() {
    let (_, plan) = planned();
    let source = two_std_line::host(two_std_line::SOURCE_HOST);
    let replacement = StdHost::new_with_config(StdHostConfig {
        host_id: HostId::from(two_std_line::SINK_HOST),
        boot_id: BootId::from("product/std-sink/boot-2"),
        offer_generation: OfferGeneration(2),
    });
    let mut current = ProductExecutionContext::new(
        vec![
            source.advertisement().clone(),
            replacement.advertisement().clone(),
        ],
        vec![
            ProductRuntime::std(source),
            ProductRuntime::std(replacement),
        ],
        vec![ConnectionBase::WebSocket],
        Vec::new(),
    )
    .unwrap();
    let error = current.execute(plan, &mut Vec::new()).err().unwrap();
    assert!(error.contains("stale Boot/offer identity"), "{error}");
}

#[test]
fn alternate_legal_placement_preserves_all_form_identities() {
    let (_, remote) = planned();
    let source = two_std_line::host(two_std_line::SOURCE_HOST);
    let sink = two_std_line::host(two_std_line::SINK_HOST);
    let context = ProductExecutionContext::new(
        vec![source.advertisement().clone(), sink.advertisement().clone()],
        vec![ProductRuntime::std(source), ProductRuntime::std(sink)],
        vec![ConnectionBase::Local, ConnectionBase::WebSocket],
        vec![two_std_line::line_offer(
            &two_std_line::host(two_std_line::SOURCE_HOST),
            &two_std_line::host(two_std_line::SINK_HOST),
        )],
    )
    .unwrap();
    let local = PlacementChoices {
        by_gear: BTreeMap::from([
            (
                GearId::from("signal-demo/pulse"),
                PlacementChoice {
                    host_id: HostId::from(two_std_line::SOURCE_HOST),
                    capability_id: "pulse-1".into(),
                },
            ),
            (
                GearId::from("signal-demo/show"),
                PlacementChoice {
                    host_id: HostId::from(two_std_line::SOURCE_HOST),
                    capability_id: "stdout-show-1".into(),
                },
            ),
        ]),
    };
    let local = context.plan_with_placements(&form(), &local).unwrap();
    assert_eq!(remote.source_document_id, local.source_document_id);
    assert_eq!(remote.checked_form_id, local.checked_form_id);
    assert_eq!(remote.expanded_form_id, local.expanded_form_id);
}
