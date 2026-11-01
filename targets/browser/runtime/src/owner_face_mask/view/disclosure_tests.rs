//! Source disclosure survives the actual Mask kernel's view projection.
use super::*;
use conduit_face::{FaceDisclosure, FaceDisclosureLevel};

fn source() -> Face {
    serde_json::from_str(include_str!(
        "../../../../../../proof/conformance/face-waist/domain-derived/open-domain-face.json"
    ))
    .unwrap()
}

fn with_disclosures(source: &Face, disclosures: Vec<FaceDisclosure>) -> Face {
    Face::new_with_semantics_and_temporal(
        source.revision,
        source.basis.clone(),
        source.subjects.clone(),
        source.relationships.clone(),
        source.properties.clone(),
        source.text.clone(),
        source.actions.clone(),
        disclosures,
        source.temporal_references.clone(),
        source.temporal_facts.clone(),
    )
    .unwrap()
    .with_composition(source.composition.clone())
    .unwrap()
    .with_interaction_context(source.interaction_context.clone())
    .unwrap()
}

fn prepare(face: &Face) -> OwnerBrowserMask {
    OwnerBrowserMask::prepare_component_fixture(
        HostBasis {
            visual_constraints: None,
            body_id: face.basis.body_id.clone().expect("domain Face has a Body"),
            host_id: "host/disclosure-proof".into(),
            boot_id: "boot/disclosure-proof".into(),
        },
        face.clone(),
        1,
        false,
    )
    .unwrap()
}

#[test]
fn absent_disclosure_is_primary_without_changing_source_or_granting_input() {
    let face = with_disclosures(&source(), vec![]);
    let before = face.clone();
    let mut mask = prepare(&face);
    let prepared = mask.view();
    assert_eq!(prepared.subjects.len(), face.subjects.len());
    assert!(prepared.subjects.iter().all(|s| s.disclosure == "Primary"));
    assert_eq!(prepared.face_id, face.identity.as_str());
    assert_eq!(prepared.face_revision, face.revision.to_string());
    let actions = serde_json::to_value(&prepared.actions).unwrap();
    assert!(!prepared.interactions_admitted);
    mask.acknowledge(Acknowledgement {
        show_id: prepared.show_id,
        face_id: prepared.face_id,
        face_revision: prepared.face_revision,
    })
    .unwrap();
    let available = mask.view();
    assert_eq!(available.show_state, "available");
    assert!(!available.interactions_admitted);
    assert_eq!(serde_json::to_value(available.actions).unwrap(), actions);
    assert_eq!(mask.face, before);
    assert_eq!(face, before);
}

#[test]
fn explicit_disclosure_is_exact_and_does_not_restrict_other_subjects() {
    let original = source();
    let original_actions = serde_json::to_value(prepare(&original).view().actions).unwrap();
    for level in [
        FaceDisclosureLevel::Primary,
        FaceDisclosureLevel::Context,
        FaceDisclosureLevel::CurrentAction,
        FaceDisclosureLevel::SelectedDetail,
        FaceDisclosureLevel::ExactProvenance,
    ] {
        let face = with_disclosures(
            &original,
            vec![FaceDisclosure {
                subject: original.subjects[0].identity.clone(),
                level,
            }],
        );
        let mask = prepare(&face);
        let view = mask.view();
        assert_eq!(view.subjects[0].disclosure, format!("{level:?}"));
        assert!(view.subjects[1..].iter().all(|s| s.disclosure == "Primary"));
        assert_eq!(
            serde_json::to_value(view.actions).unwrap(),
            original_actions
        );
        assert!(!view.interactions_admitted);
        assert_eq!(mask.face, face);
    }
}
