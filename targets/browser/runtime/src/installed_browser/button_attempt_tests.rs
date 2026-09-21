use super::*;
use conduit_form::{check_syntax_document, expand_canonical_form, parse_syntax_document};
use std::collections::BTreeMap;
fn placements() -> Vec<conduit_core::PlannedGear> {
    let (startup, profile) = crate::installed_browser::catalogs().unwrap();
    let syntax = parse_syntax_document("form timing {\n button: input/button\n attempt: time/pressed-button-attempt(maximum-presses = 3, maximum-transitions = 5, timeout-ms = 1000ms)\n derive: time/ordered-event-intervals\n button.transition > attempt.transition\n attempt.events > derive.events\n}\n");
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
    let offer = offer();
    let semantic = conduit_semantic_catalog::timed_button_attempt_semantic_contract();
    assert_eq!(offer.startup_parameters, semantic.startup_parameters);
    assert_eq!(offer.shorthand, semantic.shorthand);
    assert_eq!(offer.kind_id, semantic.kind_id);
    assert_eq!(
        offer.kind_contract_revision,
        semantic.kind_contract_revision
    );
    assert_eq!(offer.inputs, semantic.inputs);
    assert_eq!(offer.outputs, semantic.outputs);
    assert_eq!(offer.limits, semantic.limits);
    let placements = placements();
    let placement = placements
        .iter()
        .find(|p| p.implementation_id.as_str() == TIMED_BUTTON_ATTEMPT_BROWSER_IMPLEMENTATION)
        .unwrap();
    let mut store = conduit_kernel::HostedValueStore::new(32, 4096, 32768).unwrap();
    let _back = prepare(placement, &mut store).unwrap();
    assert_eq!(placement.host_calls.len(), 2);
    let observation = placement
        .host_calls
        .iter()
        .find(|binding| binding.contract_id.as_str() == TIMED_BUTTON_ATTEMPT_OBSERVE_HOST_CALL)
        .unwrap();
    assert_eq!(observation.maximum_input_bytes, 4096);
    let mut missing = placement.clone();
    missing.resources.clear();
    assert!(prepare(&missing, &mut store).is_err());
}
