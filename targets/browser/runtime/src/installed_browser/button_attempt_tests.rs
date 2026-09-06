use super::*;
use conduit_form::{check_syntax_document, expand_canonical_form, parse_syntax_document};
use conduit_kernel::{Operation, OperationAction, OperationInput, PortId, ValueStorage};
use std::collections::BTreeMap;
fn placements() -> Vec<conduit_core::PlannedGear> {
    let (startup, profile) = crate::installed_browser::catalogs().unwrap();
    let syntax = parse_syntax_document("form timing {\n button: input/button(maximum-transitions = 5)\n attempt: time/pressed-button-attempt(maximum-presses = 3, maximum-transitions = 5, timeout-ms = 1000ms)\n derive: time/ordered-event-intervals\n button.transition > attempt.transition\n attempt.events > derive.events\n}\n");
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "timing", &profile).unwrap();
    let hosts = [crate::installed_browser::advertisement(
        "timing-browser".into(),
        "timing-boot".into(),
    )];
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts).unwrap();
    conduit_planner::plan_expanded_canonical_with_options(
        &expanded,
        &hosts,
        &placements,
        &crate::installed_browser::local_bases(),
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 4096,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap()
    .fragments
    .remove(0)
    .placements
}

#[test]
fn planned_attempt_preserves_browser_bounds_and_requires_timer_admission() {
    let placements = placements();
    let placement = placements
        .iter()
        .find(|p| p.implementation_id.as_str() == TIMED_BUTTON_ATTEMPT_BROWSER_IMPLEMENTATION)
        .unwrap();
    let mut store = conduit_kernel::HostedValueStore::new(32, 4096, 32768).unwrap();
    let mut operation = prepare(placement, &mut store).unwrap();
    let bytes = conduit_semantic_catalog::button_transition_value("button/primary", true, 0)
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let value = store.store(&bytes).unwrap();
    let OperationAction::RequestHostOperation { input, .. } =
        operation.resume(OperationInput::Value {
            port: PortId(0),
            value,
        })
    else {
        panic!("attempt must request observation");
    };
    assert_eq!(input.admitted_bytes, 4096);
    let mut missing = placement.clone();
    missing.resources.clear();
    assert!(prepare(&missing, &mut store).is_err());
}
