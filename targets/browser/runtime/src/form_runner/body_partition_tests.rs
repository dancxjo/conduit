//! Exact local partition composition, not a browser or Body lifecycle proof.
use super::*;
use conduit_form::{check_syntax_document, expand_canonical_form, parse_syntax_document};
use conduit_plan_lowering::fragment_set::{lower_local_fragment_set, FragmentSetBounds};
use std::collections::BTreeMap;

fn fragment(name: &str, message: &str) -> PlanFragment {
    let source = format!("form {name} {{\n source: text/literal(\"{message}\")\n result: presentation/text\n source > result\n}}\n");
    let (startup, catalog) = crate::installed_browser::catalogs().unwrap();
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let expanded = expand_canonical_form(&checked, name, &catalog).unwrap();
    let hosts = [crate::installed_browser::advertisement(
        "body-browser".into(),
        "body-boot".into(),
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
            connection_byte_capacity: 64,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap()
    .fragments
    .remove(0)
}

#[test]
fn distinct_plans_execute_in_one_browser_kernel_without_relabeling() {
    let fragments = [fragment("first", "FIRST"), fragment("second", "SECOND")];
    let original = fragments.clone();
    let lowered = lower_local_fragment_set(
        &fragments.iter().collect::<Vec<_>>(),
        conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PROFILE,
        FragmentSetBounds {
            fragments: 2,
            nodes: MAXIMUM_BROWSER_GEARS as u16,
            cords: MAXIMUM_BROWSER_CORDS as u16,
            queue_slots: BROWSER_QUEUE_SLOTS as u16,
            value_bytes: BROWSER_TOTAL_VALUE_BYTES,
            sign_items: BROWSER_SIGN_ITEMS,
            sign_bytes: 4 * 1024 * 1024,
        },
    )
    .unwrap();
    let parts = fragments
        .iter()
        .zip(&lowered.partitions)
        .collect::<Vec<_>>();
    let mut scheduler = preparation::prepare_partition_scheduler(&parts).unwrap();
    let mut outputs = Vec::new();
    let mut nodes = Vec::new();
    loop {
        let status = drive_with_placement(&mut scheduler, |node| {
            parts.iter().find_map(|(fragment, part)| {
                let identity = part.identity.placement_for_node(node)?;
                fragment
                    .placements
                    .iter()
                    .find(|placement| &placement.placement_id == identity)
            })
        })
        .unwrap();
        match status {
            DriveStatus::Effect(pending) => {
                let BrowserHostEffect::Manifestation(value) = &pending.effect else {
                    panic!("unexpected effect")
                };
                outputs.push(value.canonical_value.clone());
                nodes.push(pending.request.node);
                complete_host_effect(&mut scheduler, &pending).unwrap();
            }
            DriveStatus::Complete => break,
            DriveStatus::Waiting { .. } => panic!("no pending effect was left incomplete"),
        }
    }
    assert_eq!(outputs, [b"FIRST".to_vec(), b"SECOND".to_vec()]);
    assert_ne!(nodes[0], nodes[1]);
    assert_eq!(fragments, original);
    exercise_session_projection(&fragments, &parts);
    for (fragment, part) in &parts {
        assert_eq!(part.identity.plan_id, fragment.plan_id);
        assert_eq!(part.identity.fragment_id, fragment.fragment_id);
    }
    // Unshifted independent numeric tables must not overwrite another Form.
    let first = lower_plan_fragment(&fragments[0]).unwrap();
    let second = lower_plan_fragment(&fragments[1]).unwrap();
    assert!(preparation::prepare_partition_scheduler(&[(&fragments[1], &first)]).is_err());
    assert!(preparation::prepare_partition_scheduler(&[]).is_err());
    assert!(preparation::prepare_partition_scheduler(&[
        (&fragments[0], &first),
        (&fragments[1], &second)
    ])
    .is_err());
}

// Internal session plumbing fixture, not a Body admission/start proof.
fn exercise_session_projection(
    fragments: &[PlanFragment],
    parts: &[(&PlanFragment, &LoweredPlanFragment)],
) {
    use super::super::{TourHostEffect, TourProgress, TourSession};
    let source = "form first {\n source: text/literal(\"FIRST\")\n result: presentation/text\n source > result\n}\n";
    let (mut session, _) = TourSession::prepare("body-browser", "body-boot", source, 1).unwrap();
    session.scheduler = preparation::prepare_partition_scheduler(parts).unwrap();
    session.pending.clear();
    session.fragments = fragments.to_vec();
    session.expanded_gears = fragments
        .iter()
        .map(|fragment| {
            fragment
                .placements
                .iter()
                .map(|gear| super::super::TourGearEvidence {
                    gear_id: gear.gear_id.as_str().into(),
                    kind_id: gear.kind_id.as_str().into(),
                    implementation_id: gear.implementation_id.as_str().into(),
                })
                .collect()
        })
        .collect();
    session.realization_backs.push(Vec::new());
    let mut effects = Vec::new();
    for _ in 0..fragments.len() {
        let TourProgress::Effect(effect) = session.poll_effect().unwrap() else {
            panic!("expected effect")
        };
        let TourHostEffect::Manifestation(effect) = *effect else {
            panic!("expected manifestation")
        };
        effects.push(effect);
    }
    assert_eq!(session.pending.len(), 2);
    assert_ne!(effects[0].placement_id, effects[1].placement_id);
    for (effect, fragment) in effects.iter().zip(fragments) {
        assert_eq!(effect.plan_id, fragment.plan_id.as_str());
        assert_eq!(effect.checked_form_id, fragment.checked_form_id.as_str());
        assert_eq!(effect.fragment_id, fragment.fragment_id.as_str());
        assert_eq!(
            effect
                .expanded_gears
                .iter()
                .map(|gear| gear.gear_id.as_str())
                .collect::<Vec<_>>(),
            fragment
                .placements
                .iter()
                .map(|gear| gear.gear_id.as_str())
                .collect::<Vec<_>>()
        );
    }
    let play = session.active_play_id.as_str().to_string();
    assert!(session
        .complete_effect(
            "stale",
            &effects[1].placement_id,
            effects[1].observation_sequence,
            None
        )
        .is_err());
    assert_eq!(session.pending.len(), 2);
    let progress = session
        .complete_effect(
            &play,
            &effects[1].placement_id,
            effects[1].observation_sequence,
            None,
        )
        .unwrap();
    assert!(matches!(progress, TourProgress::Waiting { .. }));
    assert_eq!(session.pending.len(), 1);
    assert!(session
        .complete_effect(
            &play,
            &effects[1].placement_id,
            effects[1].observation_sequence,
            None
        )
        .is_err());
    let progress = session
        .complete_effect(
            &play,
            &effects[0].placement_id,
            effects[0].observation_sequence,
            None,
        )
        .unwrap();
    assert!(matches!(progress, TourProgress::Receipt(_)));
}
