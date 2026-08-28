#![cfg(feature = "form-catalog")]

mod common;

use std::collections::BTreeMap;

use common::{host, presentation, DOM_RESOURCE, WAYLAND_RESOURCE};
use conduit_core::{bind_active_play, CapabilityId, GearId, SignId};
use conduit_form::{parse, ProfileCatalog};
use conduit_planner::{plan, PlacementChoice, PlacementChoices};
use conduit_presentation::{
    renderer_kind_definition, Manifestation, ManifestationAdmission, ManifestationError,
    ManifestationLifecycle, ManifestationSet, Presentation,
};

const SHARED_FACE_SOURCE: &str =
    "form shared-face {\n    native: presentation/renderer\n    browser: presentation/renderer\n}\n";

fn two_presenter_plan() -> (conduit_form::CheckedForm, conduit_core::Plan) {
    let mut catalog = ProfileCatalog::new();
    catalog.insert(renderer_kind_definition()).unwrap();
    let form = parse(SHARED_FACE_SOURCE, &catalog).unwrap();
    let native = host(
        "native-host",
        "native-boot",
        "renderer-wayland",
        "presentation/renderer-wayland@1",
        "patchbay-native/wayland@1",
        "presentation/base/wayland-surface@1",
        WAYLAND_RESOURCE,
    );
    let browser = host(
        "browser-host",
        "browser-boot",
        "renderer-dom-svg",
        "presentation/renderer-dom-svg@1",
        "patchbay-html/dom-svg@1",
        "presentation/base/dom-svg@1",
        DOM_RESOURCE,
    );
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([
            (
                GearId::from("shared-face/native"),
                PlacementChoice {
                    host_id: native.host_id.clone(),
                    capability_id: CapabilityId::from("renderer-wayland"),
                },
            ),
            (
                GearId::from("shared-face/browser"),
                PlacementChoice {
                    host_id: browser.host_id.clone(),
                    capability_id: CapabilityId::from("renderer-dom-svg"),
                },
            ),
        ]),
    };
    let plan = plan(&form, &[native, browser], &placements, &[]).unwrap();
    (form, plan)
}

#[test]
fn one_presentation_has_two_exact_independent_cross_host_manifestations() {
    let (form, plan) = two_presenter_plan();
    let presentation = presentation(&form, &plan);
    let admission = ManifestationAdmission::from_plan(&plan).unwrap();
    assert_eq!(admission.placement_ids.len(), 2);
    let mut manifestations = Vec::new();
    for fragment in &plan.fragments {
        let placement = &fragment.placements[0];
        let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 1);
        manifestations.push(
            Manifestation::prepared(
                &presentation,
                &plan,
                active,
                placement.placement_id.clone(),
                "patchbay/form".into(),
                format!("{}/display", fragment.host_id.as_str()),
                SignId::from(format!("{}/prepared", fragment.host_id.as_str())),
            )
            .unwrap()
            .transition(
                ManifestationLifecycle::Available,
                SignId::from(format!("{}/available", fragment.host_id.as_str())),
            )
            .unwrap(),
        );
    }
    let set =
        ManifestationSet::new(&presentation, manifestations.clone(), &plan, &admission).unwrap();
    assert!(
        ManifestationSet::new(&presentation, Vec::new(), &plan, &admission)
            .unwrap()
            .manifestations
            .is_empty()
    );
    assert_eq!(set.manifestations.len(), 2);
    assert_eq!(
        set.manifestations[0].presentation_id,
        set.manifestations[1].presentation_id
    );
    assert_ne!(
        set.manifestations[0].manifestation_id,
        set.manifestations[1].manifestation_id
    );
    assert_ne!(set.manifestations[0].host_id, set.manifestations[1].host_id);
    assert_ne!(
        set.manifestations[0].presenter_implementation_id,
        set.manifestations[1].presenter_implementation_id
    );
    for forbidden in ["native-host", "browser-host", "wayland", "dom"] {
        assert!(!SHARED_FACE_SOURCE.contains(forbidden));
    }

    let revised = revised_presentation(&presentation);
    let replacements = manifestations
        .iter()
        .map(|prior| {
            Manifestation::prepared(
                &revised,
                &plan,
                bind_active_play(
                    &plan.plan_id,
                    &prior.host_id,
                    &prior.boot_id,
                    prior.play_sequence,
                ),
                prior.placement_id.clone(),
                prior.face_subject.clone(),
                prior.target_subject.clone(),
                SignId::from(format!("{}/revision-prepared", prior.host_id.as_str())),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let revised_set = ManifestationSet::new(&revised, replacements, &plan, &admission).unwrap();
    assert_ne!(set.presentation_id, revised_set.presentation_id);
    assert_eq!(set.presentation_revision, 7);
    assert_eq!(revised_set.presentation_revision, 8);
    for (prior, replacement) in set.manifestations.iter().zip(&revised_set.manifestations) {
        assert_ne!(prior.manifestation_id, replacement.manifestation_id);
    }

    assert_negative_identity_and_bound_cases(&presentation, &plan, &admission, &manifestations);
}

fn revised_presentation(presentation: &Presentation) -> Presentation {
    let mut basis = presentation.basis.clone();
    basis.sign_ids = vec![SignId::from("patchbay/sign/revised-body-truth")];
    let mut text = presentation.text.clone();
    text[0].text = "Revised Body truth".into();
    Presentation::new_with_semantics(
        presentation.revision + 1,
        basis,
        presentation.subjects.clone(),
        presentation.relationships.clone(),
        presentation.properties.clone(),
        text,
        presentation.actions.clone(),
        presentation.disclosures.clone(),
    )
    .unwrap()
}

fn assert_negative_identity_and_bound_cases(
    presentation: &Presentation,
    plan: &conduit_core::Plan,
    admission: &ManifestationAdmission,
    manifestations: &[Manifestation],
) {
    assert_eq!(
        ManifestationSet::new(
            presentation,
            vec![manifestations[0].clone(), manifestations[0].clone()],
            plan,
            admission,
        ),
        Err(ManifestationError::DuplicateManifestation)
    );
    assert_eq!(
        ManifestationSet::new(
            presentation,
            vec![
                manifestations[0].clone();
                conduit_presentation::MAX_PRESENTATION_MANIFESTATIONS + 1
            ],
            plan,
            admission,
        ),
        Err(ManifestationError::TooManyManifestations)
    );
    let mut insufficient_admission = admission.clone();
    insufficient_admission.placement_ids.clear();
    assert_eq!(
        ManifestationSet::new(
            presentation,
            vec![manifestations[0].clone()],
            plan,
            &insufficient_admission,
        ),
        Err(ManifestationError::UnadmittedManifestation)
    );
    let mut cross_wired = manifestations[0].clone();
    cross_wired.host_id = manifestations[1].host_id.clone();
    assert_eq!(
        cross_wired.validate_against(presentation, plan),
        Err(ManifestationError::StaleIdentity)
    );
    let mut stale_boot = manifestations[0].clone();
    stale_boot.boot_id = manifestations[1].boot_id.clone();
    assert_eq!(
        stale_boot.validate_against(presentation, plan),
        Err(ManifestationError::StaleIdentity)
    );
    let mut stale_generation = manifestations[0].clone();
    stale_generation.offer_generation.0 += 1;
    assert_eq!(
        stale_generation.validate_against(presentation, plan),
        Err(ManifestationError::StaleIdentity)
    );
    let mut wrong_play = manifestations[0].clone();
    wrong_play.active_play_id = manifestations[1].active_play_id.clone();
    assert_eq!(
        wrong_play.validate_against(presentation, plan),
        Err(ManifestationError::StaleIdentity)
    );
    let mut wrong_capability = manifestations[0].clone();
    wrong_capability.presenter_capability_id = manifestations[1].presenter_capability_id.clone();
    assert_eq!(
        wrong_capability.validate_against(presentation, plan),
        Err(ManifestationError::StaleIdentity)
    );
    let mut wrong_presenter = manifestations[0].clone();
    wrong_presenter.presenter_implementation_id =
        manifestations[1].presenter_implementation_id.clone();
    assert_eq!(
        wrong_presenter.validate_against(presentation, plan),
        Err(ManifestationError::StaleIdentity)
    );
    let mut wrong_artifact = manifestations[0].clone();
    wrong_artifact.presenter_artifact_id = manifestations[1].presenter_artifact_id.clone();
    assert_eq!(
        wrong_artifact.validate_against(presentation, plan),
        Err(ManifestationError::StaleIdentity)
    );
    let mut cross_wired_sign = manifestations[0].clone();
    cross_wired_sign.signs[0].presenter_implementation_id =
        manifestations[1].presenter_implementation_id.clone();
    assert_eq!(
        cross_wired_sign.validate_against(presentation, plan),
        Err(ManifestationError::InvalidTransition)
    );
    assert!(matches!(
        Manifestation::prepared(
            presentation,
            plan,
            bind_active_play(
                &plan.plan_id,
                &manifestations[0].host_id,
                &manifestations[0].boot_id,
                manifestations[0].play_sequence,
            ),
            manifestations[0].placement_id.clone(),
            "invented/face".into(),
            "native/display".into(),
            SignId::from("invented/prepared"),
        ),
        Err(ManifestationError::UnknownFaceSubject)
    ));
}
