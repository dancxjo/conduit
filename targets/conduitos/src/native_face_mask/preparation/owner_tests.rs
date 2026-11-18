//! CPU reconstruction checks, not live routes, provider possession or Show proof.
use super::*;
use conduit_face::{FaceRole, FaceSubject};

fn fixture() -> (HostAdvertisement, Face) {
    let (mask, draft) = crate::native_face_mask::tests::fixture();
    let face = draft
        .face(&mask.birth_basis("arrival/ambient-owner"))
        .unwrap();
    let mut subjects = face.subjects;
    for (identity, role) in [
        ("fixture/current/time", "time/civil-observation"),
        ("fixture/current/weather", "weather/conditions"),
        ("fixture/current/photo", "media/image"),
    ] {
        subjects.push(FaceSubject {
            identity: identity.into(),
            role: FaceRole::Semantic(conduit_core::kind_id(role)),
            name: identity.into(),
        });
    }
    let face = Face::new_with_semantics(
        face.revision,
        face.basis,
        subjects,
        face.relationships,
        face.properties,
        face.text,
        face.actions,
        face.disclosures,
    )
    .unwrap();
    let provider = crate::product_bases::fixture_surface_provider();
    let offer = native_layout_host_advertisement(
        &"host/native".into(),
        &"boot/native".into(),
        1,
        &provider.entry,
        NativeMaskStorageProfile::Standard,
    );
    (offer, face)
}

#[test]
fn ambient_owner_reconstruction_uses_original_subjects_and_exact_plan() {
    let (offer, face) = fixture();
    let before = face.clone();
    let source = conduit_face::ambient_clock_mask_source_for_face(&face).unwrap();
    for root in ["ambient-clock-tile", "ambient-clock-mask"] {
        let (stage, configuration) = checked_ambient(&offer, root, &face).unwrap();
        let (expected, expected_configuration) = prepare_layout_component_document(
            &offer,
            &source,
            root,
            NativeMaskStorageProfile::Standard,
        )
        .unwrap();
        assert_eq!(stage.planned_mask, expected.planned_mask);
        assert_eq!(configuration, expected_configuration);
        assert!(configuration.layout.is_some());
        assert!(exact_plan(&expected.planned_mask, &stage).is_ok());
        let mut wrong = expected.planned_mask;
        wrong.mask.plot_name = "different-root".into();
        assert_eq!(exact_plan(&wrong, &stage), Err(NativeFaceMaskError::Plan));
    }
    assert_eq!(face, before);
}

#[test]
fn ambient_owner_reconstruction_requires_actual_semantics_and_layout_offer() {
    let (mut offer, mut face) = fixture();
    assert!(matches!(
        checked_ambient(&offer, "ambient-clock-mask-compact", &face),
        Err(NativeFaceMaskError::Plan)
    ));
    offer.capabilities.retain(|back| {
        back.capability_id.as_str()
            != conduit_conduitos_mask_offer::NATIVE_LAYOUT_COMPONENT_CAPABILITY
    });
    assert!(matches!(
        checked_ambient(&offer, "ambient-clock-mask", &face),
        Err(NativeFaceMaskError::Plan)
    ));
    face.subjects
        .retain(|subject| subject.identity != "fixture/current/time");
    assert!(matches!(
        checked_ambient(&offer, "ambient-clock-mask", &face),
        Err(NativeFaceMaskError::Face)
    ));
}

#[test]
fn ambiguous_semantic_binding_refuses_instead_of_selecting_first_subject() {
    let (offer, mut face) = fixture();
    face.subjects.push(FaceSubject {
        identity: "fixture/other/time".into(),
        role: FaceRole::Semantic(conduit_core::kind_id("time/civil-observation")),
        name: "Another time observation".into(),
    });
    let face = Face::new_with_semantics(
        face.revision,
        face.basis,
        face.subjects,
        face.relationships,
        face.properties,
        face.text,
        face.actions,
        face.disclosures,
    )
    .unwrap();
    assert!(matches!(
        checked_ambient(&offer, "ambient-clock-mask", &face),
        Err(NativeFaceMaskError::Face)
    ));
}
