//! CPU model/Show fixtures only; acknowledgements here are not Host effects.
use conduit_body::BodyId;
use conduit_core::{ActivePlayId, CheckedPlotId, ExpandedPlotId, PlanId, SignId, SourceDocumentId};
use conduit_face::*;
use patchbay_application::*;

#[path = "../../../../semantics/face/tests/common/mod.rs"]
mod mask_fixture;

fn setup() -> (PatchbayApplicationPort, BodyId, FaceContributionBasis) {
    let plans = conduit_patchbay_workbench_conformance::patchbay_mask_plans().unwrap();
    let port = PatchbayApplicationPort::open(
        &plans.direct_expanded,
        plans.direct.plan_id,
        PlanId::from("body-plan/canonical-fixture"),
    )
    .unwrap();
    let body = serde_json::from_str("\"body/patchbay-canonical-fixture\"").unwrap();
    let basis = FaceContributionBasis {
        checked_plot_id: CheckedPlotId::from("checked/patchbay-owner"),
        plan_id: PlanId::from("plan/patchbay-owner"),
        active_play_id: ActivePlayId::from("play/patchbay-owner"),
        required_interaction_context: None,
    };
    (port, body, basis)
}

fn face(port: &PatchbayApplicationPort, body: &BodyId, basis: &FaceContributionBasis) -> Face {
    let mut fragment = port.face_fragment(basis.clone(), body, true).unwrap();
    let revision = fragment
        .properties
        .iter()
        .find_map(|property| {
            if property.name == "application-revision" {
                if let FacePropertyValue::Count(revision) = property.value {
                    return Some(revision);
                }
            }
            None
        })
        .unwrap();
    fragment.subjects.extend([
        FaceSubject {
            identity: format!("body/{}", body.as_str()),
            role: FaceRole::Body,
            name: "Owner".into(),
        },
        FaceSubject {
            identity: format!("plot/{}", port.graph().checked_plot_id.as_str()),
            role: FaceRole::Plot,
            name: "Inspected Plot".into(),
        },
    ]);
    Face::new_with_semantics(
        revision,
        FaceBasis {
            body_id: Some(body.clone()),
            wake_id: None,
            source_document_id: Some(SourceDocumentId::from("source/patchbay-owner")),
            checked_plot_id: Some(basis.checked_plot_id.clone()),
            expanded_plot_id: Some(ExpandedPlotId::from("expanded/patchbay-owner")),
            plan_id: Some(basis.plan_id.clone()),
            active_play_id: Some(basis.active_play_id.clone()),
            sign_ids: vec![],
        },
        fragment.subjects,
        fragment.relationships,
        fragment.properties,
        fragment.text,
        fragment.actions,
        fragment.disclosures,
    )
    .unwrap()
}

fn interaction(
    face: &Face,
    show: &MaskShow,
    action: &str,
    subject: Option<&str>,
) -> FaceInteraction {
    let offered = face
        .actions
        .iter()
        .find(|offered| offered.identity == action)
        .unwrap();
    let arguments = subject
        .map(|subject| {
            vec![FaceInteractionArgument {
                name: "subject".into(),
                value_kind: UTF8_TEXT_VALUE_KIND.into(),
                value: subject.as_bytes().to_vec(),
            }]
        })
        .unwrap_or_default();
    FaceInteraction::new(face, show, action, &offered.target, arguments, 1).unwrap()
}

#[test]
fn selection_and_explicit_edit_execute_once_against_exact_owner_evidence() {
    let (mut port, body, basis) = setup();
    let initial = face(&port, &body, &basis);
    let show = mask_fixture::available_mask_show(&initial);
    let request = interaction(&initial, &show, "patchbay.plot.0", None);
    let prepared = port
        .admit_face_command(&body, basis.clone(), &initial, &show, &request)
        .unwrap();
    let duplicate = prepared.clone();
    assert_eq!(
        port.execute_face_command(prepared, &body, &basis, &initial, &show)
            .unwrap(),
        None
    );
    assert_eq!(
        port.execute_face_command(duplicate, &body, &basis, &initial, &show),
        Err(PatchbayCommandRefusal::StaleState)
    );
    let current = face(&port, &body, &basis);
    assert!(!current
        .text
        .iter()
        .any(|text| text.text.contains("no authority was inferred")));
    let current_show = mask_fixture::available_mask_show(&current);
    let subject = port.graph().subject_identities().next().unwrap().to_owned();
    let edit = interaction(
        &current,
        &current_show,
        EDIT_CURRENT_ACTION_ID,
        Some(&subject),
    );
    let prepared = port
        .admit_face_command(&body, basis.clone(), &current, &current_show, &edit)
        .unwrap();
    let expanded = port.graph().expanded_plot_id.clone();
    assert_eq!(
        port.execute_face_command(prepared, &body, &basis, &current, &current_show)
            .unwrap(),
        Some(PatchbayApplicationRequest::EditCurrent {
            expanded_plot_id: expanded,
            subject_identity: subject
        })
    );
    let requested = face(&port, &body, &basis);
    assert!(requested.text.iter().any(|text| {
        text.subject.starts_with("patchbay/document/")
            && text.text
                == "Edit requested against the exact inspected subject; no authority was inferred."
    }));
}

#[test]
fn replaced_show_refuses_before_retained_state_changes() {
    let (mut port, body, basis) = setup();
    let current = face(&port, &body, &basis);
    let show = mask_fixture::available_mask_show(&current);
    let request = interaction(&current, &show, "patchbay.plot.0", None);
    let prepared = port
        .admit_face_command(&body, basis.clone(), &current, &show, &request)
        .unwrap();
    let before = port.observe_fragment(basis.clone(), &body).unwrap();
    let replaced = show
        .transition(
            ManifestationLifecycle::Replaced,
            SignId::from("fixture/show-replaced"),
        )
        .unwrap();
    assert_eq!(
        port.execute_face_command(prepared, &body, &basis, &current, &replaced),
        Err(PatchbayCommandRefusal::InvalidInteraction(
            FaceInteractionRefusal::StaleShow
        ))
    );
    assert_eq!(port.observe_fragment(basis, &body).unwrap(), before);
}

#[test]
fn unknown_subject_and_forged_action_contract_refuse_without_mutation() {
    let (port, body, basis) = setup();
    let current = face(&port, &body, &basis);
    let show = mask_fixture::available_mask_show(&current);
    let before = port.observe_fragment(basis.clone(), &body).unwrap();
    let action = current
        .actions
        .iter()
        .find(|action| action.identity == EDIT_CURRENT_ACTION_ID)
        .unwrap();
    assert_eq!(
        FaceInteraction::new(
            &current,
            &show,
            &action.identity,
            &action.target,
            vec![FaceInteractionArgument {
                name: "subject".into(),
                value_kind: action.arguments[0].contract.value_kind.as_str().into(),
                value: b"not/a/subject".to_vec()
            }],
            1
        ),
        Err(FaceInteractionRefusal::ViolatedConstraint)
    );
    let mut actions = current.actions.clone();
    actions
        .iter_mut()
        .find(|action| action.identity == EDIT_CURRENT_ACTION_ID)
        .unwrap()
        .intent = "foreign/edit@9".into();
    let forged = Face::new_with_semantics(
        current.revision,
        current.basis.clone(),
        current.subjects.clone(),
        current.relationships.clone(),
        current.properties.clone(),
        current.text.clone(),
        actions,
        current.disclosures.clone(),
    )
    .unwrap();
    let forged_show = mask_fixture::available_mask_show(&forged);
    let subject = port.graph().subject_identities().next().unwrap();
    let request = interaction(&forged, &forged_show, EDIT_CURRENT_ACTION_ID, Some(subject));
    assert_eq!(
        port.admit_face_command(&body, basis.clone(), &forged, &forged_show, &request),
        Err(PatchbayCommandRefusal::InvalidActionContract)
    );
    assert_eq!(port.observe_fragment(basis, &body).unwrap(), before);
}

#[test]
fn canonical_wire_callback_applies_once_without_legacy_event_conversion() {
    let (mut port, body, basis) = setup();
    let current = face(&port, &body, &basis);
    let show = mask_fixture::available_mask_show(&current);
    let encoded = interaction(&current, &show, "patchbay.plot.0", None).encode();
    assert_eq!(
        port.apply_face_interaction(&encoded, &body, &basis, &current, &show),
        Ok(None)
    );
    let before = port.observe_fragment(basis.clone(), &body).unwrap();
    assert_eq!(
        port.apply_face_interaction(&encoded, &body, &basis, &current, &show),
        Err(PatchbayCommandRefusal::StaleState)
    );
    assert_eq!(port.observe_fragment(basis, &body).unwrap(), before);
}

#[test]
fn canonical_wire_callback_refuses_malformed_unacknowledged_and_foreign_owner_atomically() {
    let (mut port, body, basis) = setup();
    let current = face(&port, &body, &basis);
    let show = mask_fixture::available_mask_show(&current);
    let encoded = interaction(&current, &show, "patchbay.plot.0", None).encode();
    let mut trailing = encoded.clone();
    trailing.push(0);
    let oversized = vec![0; MAX_FACE_INTERACTION_BYTES + 1];
    let before = port.observe_fragment(basis.clone(), &body).unwrap();
    for invalid in [&[][..], &[0][..], b"{}".as_slice(), &trailing, &oversized] {
        assert_eq!(
            port.apply_face_interaction(invalid, &body, &basis, &current, &show),
            Err(PatchbayCommandRefusal::InvalidInteraction(
                FaceInteractionRefusal::MalformedEncoding
            ))
        );
        assert_eq!(port.observe_fragment(basis.clone(), &body).unwrap(), before);
    }
    let placement = show.planned_mask.show_placement();
    let unavailable = MaskShow::prepared(
        &show.planned_mask,
        &current,
        conduit_core::bind_active_play(
            &show.planned_mask.plan.plan_id,
            &placement.host_id,
            &placement.boot_id,
            2,
        ),
        current.subjects[0].identity.clone(),
        "display/unacknowledged".into(),
        SignId::from("fixture/show-unacknowledged"),
    )
    .unwrap();
    assert!(matches!(
        port.apply_face_interaction(&encoded, &body, &basis, &current, &unavailable),
        Err(PatchbayCommandRefusal::InvalidInteraction(_))
    ));
    let foreign: BodyId = serde_json::from_str("\"body/foreign-owner\"").unwrap();
    assert_eq!(
        port.apply_face_interaction(&encoded, &foreign, &basis, &current, &show),
        Err(PatchbayCommandRefusal::WrongOwner)
    );
    assert_eq!(port.observe_fragment(basis, &body).unwrap(), before);
}

#[test]
fn checked_source_selection_uses_exact_provenance_and_current_show_without_edit_authority() {
    let (port, body, basis) = setup();
    let mut port = port.with_prepared_graph_capacity(2).unwrap();
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    install_geometry_catalogs(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_vision_catalogs(&mut startup, &mut profile).unwrap();
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(include_str!("../../../../plots/vision/main.conduit")),
        &startup,
    )
    .unwrap();
    let authored =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "vision", &profile).unwrap();
    let graph = patchbay_graph::PatchbayGraph::from_authoring(&authored).unwrap();
    let expected = PatchbayResidentCommand::SelectSource {
        source_document_id: graph.source_document_id.clone(),
        checked_plot_id: graph.checked_plot_id.clone(),
        expanded_plot_id: graph.expanded_plot_id.clone(),
    };
    port.append_prepared_source_graph(graph, &"body-plan/canonical-fixture".into())
        .unwrap();
    let initial = face(&port, &body, &basis);
    let show = mask_fixture::available_mask_show(&initial);
    let request = interaction(&initial, &show, "patchbay.plot.1", None);
    let prepared = port
        .admit_face_command(&body, basis.clone(), &initial, &show, &request)
        .unwrap();
    assert_eq!(prepared.command(), &expected);
    let replay = prepared.clone();
    assert_eq!(
        port.execute_face_command(prepared, &body, &basis, &initial, &show)
            .unwrap(),
        None
    );
    assert_eq!(
        port.execute_face_command(replay, &body, &basis, &initial, &show),
        Err(PatchbayCommandRefusal::StaleState)
    );
    assert_eq!(port.selected_plan_id(), None);
    check_chunked_projection(&port, &body, &basis);
    let current = face(&port, &body, &basis);
    assert!(!current
        .actions
        .iter()
        .any(|action| action.identity == EDIT_CURRENT_ACTION_ID));
    // A former valid Edit must not acquire authority over the new selection.
    let old_edit = interaction(
        &initial,
        &show,
        EDIT_CURRENT_ACTION_ID,
        Some(port.graph().subject_identities().next().unwrap()),
    );
    let current_show = mask_fixture::available_mask_show(&current);
    assert!(port
        .admit_face_command(&body, basis, &current, &current_show, &old_edit)
        .is_err());
}

#[test]
fn chunked_contribution_matches_full_projection_and_rejects_changed_facts() {
    let (port, body, basis) = setup();
    check_chunked_projection(&port, &body, &basis);
}

#[test]
#[ignore = "bounded whole-function CPU measurement, not protected-entry acceptance"]
fn complete_contribution_cpu_screen() {
    let (port, body, basis) = setup();
    let full = face(&port, &body, &basis);
    let start = std::time::Instant::now();
    for _ in 0..1000 {
        assert!(port.matches_face_contribution(
            std::hint::black_box(&full),
            basis.clone(),
            &body,
            true
        ));
    }
    println!(
        "complete_contribution turns=1000 subjects={} properties={} elapsed_ns={}",
        full.subjects.len(),
        full.properties.len(),
        start.elapsed().as_nanos()
    );
}

#[test]
fn contribution_membership_does_not_depend_on_order_or_duplicate_position() {
    let (port, body, basis) = setup();
    let mut full = face(&port, &body, &basis);
    full.subjects.reverse();
    full.relationships.reverse();
    full.properties.reverse();
    full.text.reverse();
    full.disclosures.reverse();
    assert!(port.matches_face_contribution(&full, basis.clone(), &body, true));
    // A colliding key must not hide either an exact record or its altered peer.
    let mut changed = full.properties.last().unwrap().clone();
    changed.value = FacePropertyValue::Text("different complete fact".into());
    full.properties.insert(0, changed.clone());
    assert!(port.matches_face_contribution(&full, basis.clone(), &body, true));
    let expected = full.properties.last().unwrap().clone();
    full.properties.retain(|p| p != &expected);
    assert!(!port.matches_face_contribution(&full, basis, &body, true));
}
fn check_chunked_projection(
    port: &PatchbayApplicationPort,
    body: &BodyId,
    basis: &FaceContributionBasis,
) {
    let full = face(port, body, basis);
    assert!(port.matches_face_contribution(&full, basis.clone(), body, true));
    let fragment = port.face_fragment(basis.clone(), body, true).unwrap();
    let mut unavailable = full.clone();
    unavailable.actions = port
        .face_fragment(basis.clone(), body, false)
        .unwrap()
        .actions;
    assert!(port.matches_face_contribution(&unavailable, basis.clone(), body, false));
    assert!(!port.matches_face_contribution(&unavailable, basis.clone(), body, true));
    macro_rules! missing {
        ($field:ident) => {
            for expected in fragment
                .$field
                .iter()
                .take(1)
                .chain(fragment.$field.iter().rev().take(1))
            {
                assert!(
                    full.$field.contains(expected),
                    "absent before removal {}: {:?}",
                    stringify!($field),
                    expected
                );
                let mut changed = full.clone();
                changed.$field.retain(|item| item != expected);
                assert!(
                    !port.matches_face_contribution(&changed, basis.clone(), body, true),
                    "{}: {:?}",
                    stringify!($field),
                    expected
                );
            }
        };
    }
    missing!(subjects);
    missing!(relationships);
    missing!(properties);
    missing!(text);
    missing!(actions);
    missing!(disclosures);
    let mut forged = full.clone();
    let mut action = forged.actions[0].clone();
    action.identity = "forged-owned-action".into();
    action.target = fragment.subjects.last().unwrap().identity.clone();
    forged.actions.push(action);
    assert!(!port.matches_face_contribution(&forged, basis.clone(), body, true));
}

#[test]
fn chunked_projection_checks_exact_late_subject_facts_and_owned_actions() {
    let (port, body, basis) = setup();
    let full = face(&port, &body, &basis);
    let fragment = port.face_fragment(basis.clone(), &body, true).unwrap();
    // The final emitted graph subject is checked after earlier checkpoints.
    let expected = fragment.subjects.last().unwrap();
    let index = full.subjects.iter().position(|s| s == expected).unwrap();
    let mut renamed = full.clone();
    renamed.subjects[index].name.push_str(" forged");
    assert!(!port.matches_face_contribution(&renamed, basis.clone(), &body, true));
    let mut changed_role = full.clone();
    changed_role.subjects[index].role = conduit_face::FaceRole::Document;
    assert!(!port.matches_face_contribution(&changed_role, basis.clone(), &body, true));
    let mut extra_action = full.clone();
    let mut action = extra_action.actions[0].clone();
    action.identity = "forged-late-subject-action".into();
    action.target = expected.identity.clone();
    extra_action.actions.push(action);
    assert!(!port.matches_face_contribution(&extra_action, basis, &body, true));
}
