//! Focused production connection admission, independent of Plan hash fixtures.
extern crate alloc;
use conduit_core::*;
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "common/connection_plan.rs"]
mod fixture;
#[path = "../src/plan_connection_verification.rs"]
mod production;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

fn verify(plan: &Plan) -> bool {
    production::verify_plan_connections(plan)
}

#[test]
fn local_once_and_cross_host_twice_preserve_the_plan_order_and_bytes() {
    for cross in [false, true] {
        let plan = fixture::plan(cross, 12);
        let before = serde_json::to_vec(&plan).unwrap();
        assert!(verify(&plan));
        assert_eq!(serde_json::to_vec(&plan).unwrap(), before);
    }
    assert!(verify(&fixture::plan(false, 0)));
}

#[test]
fn missing_or_duplicated_endpoint_placements_refuse() {
    for cross in [false, true] {
        for endpoint in ["source", "sink"] {
            let mut missing = fixture::plan(cross, 1);
            for fragment in &mut missing.fragments {
                fragment
                    .placements
                    .retain(|gear| gear.placement_id.as_str() != endpoint);
            }
            assert!(!verify(&missing), "missing {endpoint}, cross={cross}");
            let mut duplicate = fixture::plan(cross, 1);
            let gear = duplicate
                .fragments
                .iter()
                .flat_map(|fragment| &fragment.placements)
                .find(|gear| gear.placement_id.as_str() == endpoint)
                .unwrap()
                .clone();
            duplicate.fragments[0].placements.push(gear);
            assert!(!verify(&duplicate), "duplicated {endpoint}, cross={cross}");
        }
    }
}

#[test]
fn occurrence_cardinality_and_conflicting_duplicate_content_refuse() {
    let mut local = fixture::plan(false, 1);
    let extra = local.fragments[0].connections[0].clone();
    local.fragments[0].connections.push(extra);
    assert!(!verify(&local));
    for copies in [1, 3] {
        let mut cross = fixture::plan(true, 1);
        cross.fragments[1].connections.clear();
        for _ in 1..copies {
            let cord = cross.fragments[0].connections[0].clone();
            cross.fragments[1].connections.push(cord);
        }
        assert!(!verify(&cross), "cross-host copies={copies}");
    }
    let mut conflict = fixture::plan(true, 1);
    conflict.fragments[1].connections[0].byte_capacity += 1;
    assert!(!verify(&conflict));
}

#[test]
fn local_cords_cannot_smuggle_selected_or_admitted_lines() {
    for selected in [false, true] {
        let mut plan = fixture::plan(false, 1);
        if selected {
            plan.fragments[0].connections[0].selected_line = Some(fixture::line());
        } else {
            plan.fragments[0].connections[0]
                .admitted_lines
                .push(fixture::line());
        }
        assert!(!verify(&plan));
    }
}

#[test]
fn selected_line_and_admitted_inventory_must_be_exact() {
    for case in 0..5 {
        let mut plan = fixture::plan(true, 1);
        fixture::change_connection(&mut plan, |cord| match case {
            0 => cord.selected_line = None,
            1 => cord.admitted_lines.clear(),
            2 => {
                cord.selected_line
                    .as_mut()
                    .unwrap()
                    .binding
                    .limits
                    .maximum_frame_bytes += 1
            }
            3 => {
                let mut line = fixture::line();
                line.binding.binding_id = "binding/other".into();
                cord.admitted_lines.push(line);
            }
            _ => {
                let mut line = fixture::line();
                line.line_id = "line/other".into();
                cord.admitted_lines.push(line);
            }
        });
        assert!(!verify(&plan), "line inventory case {case}");
    }
}

#[test]
fn alternate_admitted_lines_are_checked_even_when_not_selected() {
    let mut plan = fixture::plan(true, 1);
    fixture::change_connection(&mut plan, |cord| {
        let mut alternate = fixture::line();
        alternate.line_id = "line/alternate".into();
        alternate.binding.binding_id = "binding/alternate".into();
        alternate.binding.credential = LinkCredentialReference::None;
        alternate.binding.authority = LinkAuthorityReference::ProcessOwned;
        cord.admitted_lines.push(alternate);
    });
    assert!(verify(&plan));
    fixture::change_connection(&mut plan, |cord| {
        cord.admitted_lines[1].binding.sink.boot_id = "boot/stale".into();
    });
    assert!(!verify(&plan));
}

#[test]
fn every_admitted_line_keeps_endpoint_authority_identity_and_limits_checks() {
    for case in 0..17 {
        let mut plan = fixture::plan(true, 1);
        fixture::change_connection(&mut plan, |cord| {
            let candidate = &mut cord.admitted_lines[0];
            let binding = &mut candidate.binding;
            match case {
                0 => candidate.line_id = "".into(),
                1 => binding.binding_id = "".into(),
                2 => binding.source.host_id = "host/foreign".into(),
                3 => binding.source.boot_id = "boot/stale".into(),
                4 => binding.sink.host_id = "host/foreign".into(),
                5 => binding.sink.boot_id = "boot/stale".into(),
                6 => binding.source.endpoint_id = "".into(),
                7 => binding.sink.endpoint_id = binding.source.endpoint_id.clone(),
                8 => binding.base = LOCAL_BASE_IMPLEMENTATION_ID.into(),
                9 => binding.base_instance_id = "".into(),
                10 => binding.credential = LinkCredentialReference::Opaque("".into()),
                11 => binding.authority = LinkAuthorityReference::Grant("".into()),
                12 => binding.limits.maximum_in_flight_items = 0,
                13 => binding.limits.maximum_payload_bytes = 7,
                14 => binding.limits.maximum_buffered_bytes = 7,
                15 => binding.limits.maximum_frame_bytes = 7,
                _ => binding.binding_id = candidate.line_id.as_str().into(),
            }
            // Keep selected membership valid, isolating admitted-line validation.
            cord.selected_line = Some(candidate.clone());
        });
        assert!(!verify(&plan), "admitted-line case {case}");
    }
}

#[test]
fn actual_verifier_uses_only_one_borrowed_connection_vector() {
    let observations = [false, true].map(|cross| {
        let plan = fixture::plan(cross, 12);
        let occurrences = plan
            .fragments
            .iter()
            .map(|fragment| fragment.connections.len())
            .sum::<usize>();
        let (accepted, work) = allocation_probe::observe(|| verify(&plan));
        assert!(accepted);
        std::eprintln!("cross_host={cross} occurrences={occurrences} allocations={} reallocations={} requested_bytes={}", work.allocations, work.reallocations, work.requested_bytes);
        (occurrences, work)
    });
    for (occurrences, work) in observations {
        assert_eq!(work.allocations, 1, "{work:?}");
        // Includes any geometric growth of the one borrowed-pointer inventory.
        assert!(
            work.requested_bytes <= 4 * occurrences * core::mem::size_of::<&PlannedConnection>(),
            "{work:?}"
        );
    }
}
