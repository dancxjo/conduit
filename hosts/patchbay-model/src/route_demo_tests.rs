use super::*;

#[test]
fn document_distinguishes_replan_from_same_plan_fallback() {
    let demo = DistributedRouteDemo::build().expect("route demo");
    let text = demo.lines().join("\n");
    assert!(text.contains("semantic-host-facts=none semantic-carrier-facts=none"));
    assert!(text.contains("PLAN-A replan-required"));
    assert!(text.contains("OUTCOME replan=true prior-plan="));
    assert!(text.contains("PLAN-B predeclared-fallback"));
    assert!(text.contains("OUTCOME replan=false same-plan="));
    assert!(text.contains("REFUSED route=ambient/unplanned-wifi"));
    assert!(text.contains("patchbay-native/std-realization"));
    assert!(text.contains(conduit_signal::DISTRIBUTED_BROWSER_HOST_ID));
}

#[test]
fn candidate_order_changes_exact_plan_identity() {
    let host = HostId::from("patchbay-native/std-realization");
    let boot = BootId::from("patchbay-native/std-boot-1");
    let usb_first = planned(
        &[USB_ROUTE, conduit_signal::DISTRIBUTED_LINK_BINDING_ID],
        &host,
        &boot,
    )
    .unwrap();
    let websocket_first = planned(
        &[conduit_signal::DISTRIBUTED_LINK_BINDING_ID, USB_ROUTE],
        &host,
        &boot,
    )
    .unwrap();
    assert_ne!(usb_first.plan_id, websocket_first.plan_id);
}

#[test]
fn visual_and_linear_views_preserve_one_typed_identity_set() {
    let demo = DistributedRouteDemo::build().expect("route demo");
    let facts = demo.presentation();
    let visual = demo.visual_lines().join("\n");
    let linear = demo.linear_lines().join("\n");
    let exact_identities = [
        facts.source_document_id.as_str(),
        facts.checked_form_id.as_str(),
        facts.new_plan.prior.plan_id.as_str(),
        facts.new_plan.prior.connection_id.as_str(),
        facts.new_plan.replacement_plan_id.as_str(),
        facts.new_plan.unavailable_evidence_id.as_str(),
        facts.new_plan.unsatisfied_evidence_id.as_str(),
        facts.new_plan.planning_request_evidence_id.as_str(),
        facts.new_plan.planning_success_evidence_id.as_str(),
        facts.new_plan.installed_evidence_id.as_str(),
        facts.same_plan.plan.plan_id.as_str(),
        facts.same_plan.plan.connection_id.as_str(),
        facts.same_plan.unavailable_evidence_id.as_str(),
        facts.same_plan.selection_evidence_id.as_str(),
        facts.refused.observation_evidence_id.as_str(),
    ];
    for identity in exact_identities {
        assert!(visual.contains(identity), "visual omitted {identity}");
        assert!(linear.contains(identity), "linear omitted {identity}");
    }
    for candidate in facts
        .new_plan
        .prior
        .candidates
        .iter()
        .chain(&facts.same_plan.plan.candidates)
    {
        for identity in [
            candidate.binding_id.as_str(),
            candidate.provider_instance_id.as_str(),
        ] {
            assert!(visual.contains(identity), "visual omitted {identity}");
            assert!(linear.contains(identity), "linear omitted {identity}");
        }
    }
    assert!(visual.contains("Plan C unchanged"));
    assert!(linear.contains("Plan identity did not change"));
    assert!(visual.contains("UNPLANNED ROUTE refused=ambient Wi-Fi"));
    assert!(linear.contains("ambient Wi-Fi, was refused"));
}
