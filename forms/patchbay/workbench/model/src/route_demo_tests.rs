use super::*;
use conduit_core::{BootId, HostId, OfferGeneration};

#[test]
fn document_distinguishes_replan_from_same_plan_fallback() {
    let demo = DistributedRouteDemo::build().expect("route demo");
    let text = demo.lines().join("\n");
    assert!(text.contains("semantic-host-facts=none semantic-line-facts=none"));
    assert!(text.contains("PLAN-A replan-required"));
    assert!(text.contains("OUTCOME replan=true prior-plan="));
    assert!(text.contains("PLAN-B predeclared-fallback"));
    assert!(text.contains("OUTCOME replan=false same-plan="));
    assert!(text.contains("REFUSED route=ambient/unplanned-wifi"));
    assert!(text.contains("patchbay-native/std-realization"));
    assert!(text.contains(conduit_signal_conformance::DISTRIBUTED_BROWSER_HOST_ID));
}

#[test]
fn candidate_order_changes_exact_plan_identity() {
    let host = HostId::from("patchbay-native/std-realization");
    let boot = BootId::from("patchbay-native/std-boot-1");
    let source = native_advertisement_for_test(host, boot);
    let usb_first = planned(
        &[USB_LINE, conduit_signal_conformance::DISTRIBUTED_LINE_ID],
        &source,
    )
    .unwrap();
    let websocket_first = planned(
        &[conduit_signal_conformance::DISTRIBUTED_LINE_ID, USB_LINE],
        &source,
    )
    .unwrap();
    assert_ne!(usb_first.plan_id, websocket_first.plan_id);
}

fn native_advertisement_for_test(host_id: HostId, boot_id: BootId) -> HostAdvertisement {
    use conduit_std_host::{StdHost, StdHostComposition, StdHostConfig};
    StdHost::new_with_composition(
        StdHostConfig {
            host_id,
            boot_id,
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::minimal().with_signal(),
    )
    .advertisement()
    .clone()
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
        facts.new_plan.unavailable_sign_id.as_str(),
        facts.new_plan.unsatisfied_sign_id.as_str(),
        facts.new_plan.planning_request_sign_id.as_str(),
        facts.new_plan.planning_success_sign_id.as_str(),
        facts.new_plan.installed_sign_id.as_str(),
        facts.same_plan.plan.plan_id.as_str(),
        facts.same_plan.plan.connection_id.as_str(),
        facts.same_plan.unavailable_sign_id.as_str(),
        facts.same_plan.selection_sign_id.as_str(),
        facts.refused.observation_sign_id.as_str(),
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
            candidate.base_instance_id.as_str(),
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

#[test]
fn projection_uses_typed_bases_and_exact_validated_event_sign() {
    let demo = DistributedRouteDemo::build().expect("route demo");
    let facts = demo.presentation();
    assert_eq!(
        facts.new_plan.prior.candidates[0].base,
        BaseImplementationId::from("conduit.base/usb-cdc-acm@1")
    );
    assert_eq!(
        facts.same_plan.plan.candidates[1].base,
        BaseImplementationId::from("conduit.base/websocket-rfc6455@1")
    );

    let [link, unsatisfied, requested, succeeded, installed, selected] = demo.control_events()
    else {
        panic!("route demo must retain its six validated control events");
    };
    let ControlLoopEvent::LineBecameUnavailable {
        observation_sign_id,
        ..
    } = link
    else {
        panic!("first event must be link loss");
    };
    assert_eq!(&facts.new_plan.unavailable_sign_id, observation_sign_id);
    let ControlLoopEvent::PlayBecameUnsatisfied { sign_id, .. } = unsatisfied else {
        panic!("second event must be an unsatisfied Play");
    };
    assert_eq!(&facts.new_plan.unsatisfied_sign_id, sign_id);
    let ControlLoopEvent::PlanningRequested {
        request_sign_id, ..
    } = requested
    else {
        panic!("third event must be planning request");
    };
    assert_eq!(&facts.new_plan.planning_request_sign_id, request_sign_id);
    let ControlLoopEvent::PlanningSucceeded { sign_id, .. } = succeeded else {
        panic!("fourth event must be planning success");
    };
    assert_eq!(&facts.new_plan.planning_success_sign_id, sign_id);
    let ControlLoopEvent::PlanSuperseded { sign_id, .. } = installed else {
        panic!("fifth event must install the replacement Plan");
    };
    assert_eq!(&facts.new_plan.installed_sign_id, sign_id);
    let ControlLoopEvent::LineSelectionChanged {
        observation_sign_id,
        ..
    } = selected
    else {
        panic!("sixth event must be same-Plan selection");
    };
    assert_eq!(&facts.same_plan.selection_sign_id, observation_sign_id);
    assert_eq!(&facts.same_plan.unavailable_sign_id, observation_sign_id);
}
