//! Correlated concurrent platform effects through the existing browser kernel.
use super::*;
use conduit_form::{check_syntax_document, expand_canonical_form, parse_syntax_document};
use std::collections::BTreeMap;

#[test]
fn button_completion_progresses_while_an_independent_timer_remains_pending() {
    let source = "form concurrent {\n button: input/button(maximum-transitions = 1)\n state: input/button-indicator-state\n indicator: presentation/indicator-state\n clock: time/every(freq = 100ms)\n count: state/count(start = 0)\n show: presentation/count(maximum-values = 5)\n button > state > indicator\n clock.tick > count.bump\n count.value > show.value\n}\n";
    let (startup, catalog) = crate::installed_browser::catalogs().unwrap();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "concurrent", &catalog).unwrap();
    let hosts = [crate::installed_browser::advertisement(
        "concurrent-browser".into(),
        "concurrent-boot".into(),
    )];
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts).unwrap();
    let fragment = conduit_planner::plan_expanded_canonical_with_options(
        &expanded,
        &hosts,
        &placements,
        &crate::installed_browser::local_bases(),
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: MAXIMUM_BROWSER_VALUE_BYTES as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap()
    .fragments
    .remove(0);
    let lowered = lower_plan_fragment(&fragment).unwrap();
    validate_envelope(&fragment, &lowered, false).unwrap();
    let mut scheduler = prepare_scheduler(&fragment, &lowered).unwrap();
    let mut timer = None;
    let mut button = None;
    for _ in 0..4 {
        let effect = match drive(&mut scheduler, &fragment).unwrap() {
            DriveStatus::Effect(effect) => effect,
            DriveStatus::Waiting { pending_effects: 2 } => break,
            _ => panic!("both independent effects must be yielded before waiting"),
        };
        match effect.effect {
            BrowserHostEffect::Timer { .. } => timer = Some(effect),
            BrowserHostEffect::ButtonTransition => button = Some(effect),
            BrowserHostEffect::Manifestation(_) => {
                complete_host_effect(&mut scheduler, &effect).unwrap()
            }
            _ => panic!("unexpected effect"),
        }
    }
    let timer = timer.unwrap();
    let button = button.unwrap();
    // Request sequences are node-scoped; both can legitimately be zero.
    assert_eq!(timer.request.request, button.request.request);
    assert_ne!(timer.request.node, button.request.node);
    assert!(matches!(
        drive(&mut scheduler, &fragment).unwrap(),
        DriveStatus::Waiting { pending_effects: 2 }
    ));
    let bytes = conduit_semantic_catalog::button_transition_value("button/primary", true, 0)
        .unwrap()
        .canonical_bytes()
        .unwrap();
    complete_host_effect_with_output(&mut scheduler, &button, &bytes).unwrap();
    let available = available_slots(&mut scheduler);
    for _ in 0..4 {
        assert!(complete_host_effect_with_output(&mut scheduler, &button, &bytes).is_err());
    }
    assert_eq!(available_slots(&mut scheduler), available);
    let DriveStatus::Effect(presentation) = drive(&mut scheduler, &fragment).unwrap() else {
        panic!("button must manifest without completing the timer");
    };
    let BrowserHostEffect::Manifestation(value) = &presentation.effect else {
        panic!("expected indicator")
    };
    assert_eq!(value.canonical_value, [1]);
    complete_host_effect(&mut scheduler, &presentation).unwrap();
    assert!(matches!(
        drive(&mut scheduler, &fragment).unwrap(),
        DriveStatus::Waiting { pending_effects: 1 }
    ));
    scheduler.cancel().unwrap();
    assert!(complete_host_effect(&mut scheduler, &timer).is_err());
}

// Test-only capacity inspection; release every probe before returning.
fn available_slots(scheduler: &mut TourScheduler) -> usize {
    let mut probes = Vec::new();
    while let Ok(value) = scheduler.store_host_value(&[0]) {
        probes.push(value);
    }
    let count = probes.len();
    for value in probes {
        scheduler.discard_host_value(value).unwrap();
    }
    count
}
