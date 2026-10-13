//! Checked-source custody and explicit consumption of a real retained Face.
extern crate std;
use super::*;
use crate::{CheckedSourcePlanInputs, Presentation};
use alloc::{collections::BTreeMap, vec};
use conduit_core::{ConfigurationValue, HostAdvertisement, PortDirection};
use conduit_planner::{ConnectionQueueLimits, ForeBoundaryKey, PlanningOptions};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    CanonicalBackCatalog, PortableExpressionProgram, ProfileCatalog, StartupCatalog,
};

fn producer(name: &str) -> PreparedSpokenTemplate {
    let checked = check_syntax_document(
        &parse_syntax_document(SPOKEN_TEMPLATE_LOOKS_SOURCE),
        &StartupCatalog::new(),
    )
    .unwrap();
    let profiles = ProfileCatalog::new();
    let expanded = expand_canonical_plot_for_authoring(&checked, name, &profiles).unwrap();
    let gear = &expanded.expanded.gears[0];
    let ConfigurationValue::Text(program) = &gear.configuration[0].value else {
        panic!("program");
    };
    let program = PortableExpressionProgram::from_canonical_hex(program).unwrap();
    let hosts = [HostAdvertisement {
        protocol_version: conduit_core::PROTOCOL_VERSION,
        host_id: "fixture/spoken-template".into(),
        boot_id: "fixture/spoken-template/boot".into(),
        offer_generation: conduit_core::OfferGeneration(1),
        profile: "fixture/spoken-template".into(),
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: vec![conduit_std_offers::pure_expression_std_offer(
            &program,
            gear.inputs[0].temporal,
        )
        .unwrap()],
    }];
    let placements =
        conduit_planner::default_expanded_placements(&expanded.expanded, &hosts).unwrap();
    let boundaries: BTreeMap<_, _> = expanded
        .input_bindings
        .iter()
        .map(|binding| (PortDirection::Input, binding))
        .chain(
            expanded
                .output_bindings
                .iter()
                .map(|binding| (PortDirection::Output, binding)),
        )
        .map(|(direction, binding)| {
            (
                ForeBoundaryKey {
                    direction,
                    front_port_id: binding.front_port_id.clone(),
                    track: binding.track,
                },
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: 128 * 1024,
                },
            )
        })
        .collect();
    let empty = BTreeMap::new();
    let lines = BTreeMap::new();
    let options = || PlanningOptions {
        connection_bases: &empty,
        line_candidates: &lines,
        connection_item_capacity: 1,
        connection_byte_capacity: 128 * 1024,
        authority_grants: &[],
        protected_resource_grants: &[],
        line_offers: &[],
    };
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &expanded,
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
        options(),
        &boundaries,
    )
    .unwrap();
    let prepared = crate::prepare_checked_source_plan(
        &checked,
        &expanded,
        &profiles,
        &CanonicalBackCatalog::new(),
        &plan,
        CheckedSourcePlanInputs {
            hosts: &hosts,
            placements: &placements,
            bases: &["conduit.base/local@1".into()],
            options: options(),
            boundary_limits: &boundaries,
        },
    )
    .unwrap();
    prepare_checked_spoken_template(&prepared).unwrap()
}

#[test]
fn two_template_sources_have_distinct_checked_plots_and_exact_selected_plans() {
    let brief = producer("spoken/brief-template");
    let expanded = producer("spoken/expanded-template");
    assert_eq!(brief.identity(), "spoken/brief@1");
    assert_eq!(expanded.identity(), "spoken/expanded@1");
    assert_ne!(brief.producer_plot(), expanded.producer_plot());
    assert_ne!(brief.producer_plan_id(), expanded.producer_plan_id());
}

#[test]
#[ignore = "requires explicit actual retained Todo Face; never manufactures owner state"]
fn actual_retained_face_preserves_essential_clauses_in_both_checked_looks() {
    let path = std::path::PathBuf::from(
        std::env::var_os("CONDUIT_SPOKEN_TEMPLATE_FACE").expect("actual retained Face path"),
    );
    assert!(path.is_absolute());
    let record: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let face: Presentation =
        serde_json::from_value(record.get("presentation").unwrap_or(&record).clone()).unwrap();
    assert!(face.basis.body_id.is_some());
    let original = face.clone();
    let observed_plan = crate::plan_face_utterances(&face).unwrap();
    std::eprintln!(
        "actual retained aural projection: {} clauses, {} text bytes",
        observed_plan.clauses.len(),
        observed_plan
            .clauses
            .iter()
            .map(|clause| clause.text.len() + 1)
            .sum::<usize>()
    );
    let slots = prepare_spoken_template_slots(&face, &[]).unwrap();
    let brief = producer("spoken/brief-template")
        .evaluate(&face, &slots)
        .unwrap();
    let expanded = producer("spoken/expanded-template")
        .evaluate(&face, &slots)
        .unwrap();
    assert_ne!(brief.wording, expanded.wording);
    assert_eq!(brief.source_face_identity, expanded.source_face_identity);
    assert_eq!(brief.source_face_revision, expanded.source_face_revision);
    assert_eq!(
        brief.selected_clause_indices,
        expanded.selected_clause_indices
    );
    assert_eq!(brief.selected_action_ids, expanded.selected_action_ids);
    assert_eq!(brief.utterance_plan_digest, expanded.utterance_plan_digest);
    let plan = crate::plan_face_utterances(&face).unwrap();
    let mut all_indices = brief.selected_clause_indices.clone();
    all_indices.sort_unstable();
    assert_eq!(
        all_indices,
        (0..plan.clauses.len() as u32).collect::<alloc::vec::Vec<_>>(),
        "every projected clause, including unavailable actions and unknown fallback, must survive"
    );
    for clause in &plan.clauses {
        assert!(brief.wording.contains(&clause.text));
        assert!(expanded.wording.contains(&clause.text));
    }

    for clause in slots.values.iter().filter(|clause| !clause.is_empty()) {
        assert!(brief.segments.contains(clause));
        assert!(expanded.segments.contains(clause));
    }
    assert_eq!(face, original);
    let mut stale = face.clone();
    stale.revision += 1;
    assert!(matches!(
        producer("spoken/brief-template").evaluate(&stale, &slots),
        Err(SpokenTemplateRefusal::StaleFace)
    ));
    let mut invented_action = face.clone();
    invented_action.actions[0].name = "Invented authority".into();
    assert!(producer("spoken/brief-template")
        .evaluate(&invented_action, &slots)
        .is_err());
    let mut lost_status = face.clone();
    lost_status.text.retain(|text| {
        !lost_status.subjects.iter().any(|subject| {
            subject.identity == text.subject && subject.role == crate::PresentationRole::Status
        })
    });
    // This omission counterexample is a valid differently projected Face,
    // not merely a corrupt content identity.
    let lost_status = lost_status
        .with_interaction_context(face.interaction_context.clone())
        .unwrap();
    assert!(matches!(
        prepare_spoken_template_slots(&lost_status, &[]),
        Err(SpokenTemplateRefusal::MissingEssential)
    ));
    let mut oversized = face.clone();
    let index = oversized
        .text
        .iter()
        .position(|text| {
            oversized.subjects.iter().any(|subject| {
                subject.identity == text.subject && subject.role == crate::PresentationRole::Status
            })
        })
        .unwrap();
    oversized.text[index].text = "x".repeat(MAX_SPOKEN_TEMPLATE_SLOT_BYTES + 1);
    assert!(
        prepare_spoken_template_slots(&oversized, &[]).is_err(),
        "must refuse, not truncate essential wording"
    );
    let denied = face.text.iter().position(|word| {
        face.disclosures.iter().any(|disclosure| {
            disclosure.subject == word.subject
                && !matches!(
                    disclosure.level,
                    crate::PresentationDisclosureLevel::Primary
                        | crate::PresentationDisclosureLevel::Context
                )
        })
    });
    if let Some(index) = denied {
        assert!(matches!(
            prepare_spoken_template_slots(&face, &[index as u32]),
            Err(SpokenTemplateRefusal::Disclosure)
        ));
    }
}
