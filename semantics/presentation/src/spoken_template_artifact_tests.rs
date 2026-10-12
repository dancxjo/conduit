//! Synthetic semantic Face and checked producer metadata; no audio/Body effect claim.
extern crate std;
use super::*;
use crate::{
    prepare_checked_spoken_template, prepare_spoken_template_slots, CheckedSourcePlanInputs,
    PreparedSpokenTemplate, PresentationAction, PresentationActionAvailability, PresentationBasis,
    PresentationDisclosureLevel, PresentationRole, PresentationSubject, PresentationText,
};
use alloc::{collections::BTreeMap, vec};
use conduit_core::{ConfigurationValue, HostAdvertisement, PortDirection};
use conduit_planner::{ConnectionQueueLimits, ForeBoundaryKey, PlanningOptions};
use conduit_plot::{CanonicalBackCatalog, PortableExpressionProgram};

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

fn face() -> Presentation {
    Presentation::new_with_semantics(
        1,
        PresentationBasis {
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        vec![
            PresentationSubject {
                identity: "todo/list".into(),
                role: PresentationRole::Collection,
                name: "Tasks".into(),
            },
            PresentationSubject {
                identity: "todo/status".into(),
                role: PresentationRole::Status,
                name: "Status".into(),
            },
            PresentationSubject {
                identity: "todo/item".into(),
                role: PresentationRole::Item,
                name: "Task".into(),
            },
        ],
        vec![],
        vec![],
        vec![
            PresentationText {
                subject: "todo/status".into(),
                text: "Current state is retained. ".repeat(20),
            },
            PresentationText {
                subject: "todo/item".into(),
                text: "Read naïve résumé carefully. ".repeat(12),
            },
        ],
        vec![PresentationAction {
            identity: "todo/add".into(),
            intent: "todo/add".into(),
            target: "todo/list".into(),
            name: "Add task".into(),
            arguments: vec![],
            disclosure: PresentationDisclosureLevel::Primary,
            availability: PresentationActionAvailability::Available,
        }],
        vec![],
    )
    .unwrap()
}
fn evidence(face: &Presentation, name: &str) -> SpokenTemplateOpeningEvidence {
    let wording = producer(name)
        .evaluate(face, &prepare_spoken_template_slots(face, &[]).unwrap())
        .unwrap();
    let mut end = wording.wording.len().min(256);
    while !wording.wording.is_char_boundary(end) {
        end -= 1;
    }
    SpokenTemplateOpeningEvidence {
        opening_wording: wording.wording[..end].into(),
        frame_count: wording.wording.len().div_ceil(256) as u16,
        wording,
    }
}

#[test]
fn both_checked_template_metadata_bind_same_face_and_bounded_opening_only() {
    let face = face();
    let brief = evidence(&face, "spoken/brief-template");
    let expanded = evidence(&face, "spoken/expanded-template");
    brief.validate(&face).unwrap();
    expanded.validate(&face).unwrap();
    assert_eq!(
        brief.wording.source_face_identity,
        expanded.wording.source_face_identity
    );
    assert_ne!(brief.wording.wording, expanded.wording.wording);
    assert!(brief.wording.wording.len() > brief.opening_wording.len());
    assert!(brief.frame_count > 1);
    let encoded = serde_json::to_vec(&brief).unwrap();
    let decoded: SpokenTemplateOpeningEvidence = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(decoded, brief);
}

#[test]
fn stale_face_source_identity_revision_plot_and_digests_refuse() {
    let face = face();
    let original = evidence(&face, "spoken/brief-template");
    for variant in 0..8 {
        let mut changed = original.clone();
        match variant {
            0 => changed.wording.source_face_identity = "forged/face".into(),
            1 => changed.wording.source_face_revision += 1,
            2 => changed.wording.template_identity = "spoken/forged@1".into(),
            3 => changed.wording.template_revision += 1,
            4 => changed.wording.template_source_digest[0] ^= 1,
            5 => changed.wording.producer_plot.checked_plot_id = "forged/plot".into(),
            6 => changed.wording.utterance_plan_digest[0] ^= 1,
            _ => changed.wording.wording_digest[0] ^= 1,
        }
        assert!(changed.validate(&face).is_err());
    }
}

#[test]
fn dropped_coverage_actions_wrong_prefix_and_unbounded_frame_claims_refuse() {
    let face = face();
    let original = evidence(&face, "spoken/expanded-template");
    for variant in 0..9 {
        let mut changed = original.clone();
        match variant {
            0 => {
                changed.wording.selected_clause_indices.pop();
            }
            1 => changed.wording.selected_action_ids.clear(),
            2 => changed.opening_wording = "Invented speech".into(),
            3 => changed.opening_wording.clear(),
            4 => changed.opening_wording = changed.wording.wording[..257].into(),
            5 => changed.frame_count = 0,
            6 => changed.frame_count = 65,
            7 => changed.frame_count = 1,
            _ => {
                changed.wording.segments.pop();
            }
        }
        assert!(changed.validate(&face).is_err());
    }
}

#[test]
fn rehashed_wording_cannot_drop_selected_face_content() {
    let face = face();
    let mut changed = evidence(&face, "spoken/expanded-template");
    let plan = crate::plan_face_utterances(&face).unwrap();
    let essential = plan
        .clauses
        .iter()
        .find(|clause| clause.text.contains("Read naïve résumé"))
        .unwrap();
    for segment in &mut changed.wording.segments {
        *segment = segment.replace(&essential.text, "");
    }
    changed.wording.segments.retain(|words| !words.is_empty());
    changed.wording.wording = changed.wording.segments.join(" ");
    changed.wording.wording_digest = Sha256::digest(changed.wording.wording.as_bytes()).into();
    let mut end = changed.wording.wording.len().min(256);
    while !changed.wording.wording.is_char_boundary(end) {
        end -= 1;
    }
    changed.opening_wording = changed.wording.wording[..end].into();
    changed.frame_count = changed.wording.wording.len().div_ceil(256) as u16;
    assert_eq!(
        changed.validate(&face),
        Err(SpokenTemplateOpeningRefusal::Coverage)
    );
}

#[test]
fn rehashed_appended_claim_is_not_the_checked_template_output() {
    let face = face();
    let mut changed = evidence(&face, "spoken/brief-template");
    changed
        .wording
        .segments
        .push("Invented authority: every task was completed.".into());
    changed.wording.wording = changed.wording.segments.join(" ");
    changed.wording.wording_digest = Sha256::digest(changed.wording.wording.as_bytes()).into();
    changed.frame_count = changed.wording.wording.len().div_ceil(256) as u16;
    // Every original clause/action/reference and digest still corresponds;
    // only actual static checked-source replay can reject the added prose.
    assert_eq!(
        changed.validate(&face),
        Err(SpokenTemplateOpeningRefusal::Coverage)
    );
}
