#![cfg(feature = "plot-catalog")]

mod common;

use std::collections::BTreeMap;

use common::{face, host, DOM_RESOURCE, WAYLAND_RESOURCE};
use conduit_core::{bind_active_play, CapabilityId, GearId, SignId};
use conduit_face::{
    renderer_kind_projection, Face, Manifestation, ManifestationAdmission, ManifestationError,
    ManifestationLifecycle, ManifestationSet,
};
use conduit_planner::{plan, PlacementChoice, PlacementChoices};
use conduit_plot::{parse, ProfileCatalog};

const SHARED_FACE_SOURCE: &str =
    "plot shared-front {\n    native: face/renderer(\"Rendered Face\")\n    browser: face/renderer(\"Rendered Face\")\n}\n";

fn two_presenter_plan() -> (conduit_plot::CheckedPlot, conduit_core::Plan) {
    let mut catalog = ProfileCatalog::new();
    catalog.insert(renderer_kind_projection()).unwrap();
    let plot = parse(SHARED_FACE_SOURCE, &catalog).unwrap();
    let native = host(
        "native-host",
        "native-boot",
        "renderer-wayland",
        "face/renderer-wayland@1",
        "patchbay-native/wayland@1",
        "face/base/wayland-surface@1",
        WAYLAND_RESOURCE,
    );
    let browser = host(
        "browser-host",
        "browser-boot",
        "renderer-dom-svg",
        "face/renderer-dom-svg@1",
        "patchbay-html/dom-svg@1",
        "face/base/dom-svg@1",
        DOM_RESOURCE,
    );
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([
            (
                GearId::from("shared-front/native"),
                PlacementChoice {
                    host_id: native.host_id.clone(),
                    capability_id: CapabilityId::from("renderer-wayland"),
                },
            ),
            (
                GearId::from("shared-front/browser"),
                PlacementChoice {
                    host_id: browser.host_id.clone(),
                    capability_id: CapabilityId::from("renderer-dom-svg"),
                },
            ),
        ]),
    };
    let plan = plan(&plot, &[native, browser], &placements, &[]).unwrap();
    (plot, plan)
}

#[test]
fn one_face_has_two_exact_independent_cross_host_manifestations() {
    let (plot, plan) = two_presenter_plan();
    let face = face(&plot, &plan);
    let admission = ManifestationAdmission::from_plan(&plan).unwrap();
    assert_eq!(admission.placement_ids.len(), 2);
    let mut manifestations = Vec::new();
    for fragment in &plan.fragments {
        let placement = &fragment.placements[0];
        let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 1);
        manifestations.push(
            Manifestation::prepared(
                &face,
                &plan,
                active,
                placement.placement_id.clone(),
                "patchbay/plot".into(),
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
    let set = ManifestationSet::new(&face, manifestations.clone(), &plan, &admission).unwrap();
    assert!(ManifestationSet::new(&face, Vec::new(), &plan, &admission)
        .unwrap()
        .manifestations
        .is_empty());
    assert_eq!(set.manifestations.len(), 2);
    assert_eq!(set.manifestations[0].face_id, set.manifestations[1].face_id);
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

    let revised = revised_face(&face);
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
                prior.front_subject.clone(),
                prior.target_subject.clone(),
                SignId::from(format!("{}/revision-prepared", prior.host_id.as_str())),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let revised_set = ManifestationSet::new(&revised, replacements, &plan, &admission).unwrap();
    assert_ne!(set.face_id, revised_set.face_id);
    assert_eq!(set.face_revision, 7);
    assert_eq!(revised_set.face_revision, 8);
    for (prior, replacement) in set.manifestations.iter().zip(&revised_set.manifestations) {
        assert_ne!(prior.manifestation_id, replacement.manifestation_id);
    }

    assert_negative_identity_and_bound_cases(&face, &plan, &admission, &manifestations);
}

fn revised_face(face: &Face) -> Face {
    let mut basis = face.basis.clone();
    basis.sign_ids = vec![SignId::from("patchbay/sign/revised-body-truth")];
    let mut text = face.text.clone();
    text[0].text = "Revised Body truth".into();
    Face::new_with_semantics(
        face.revision + 1,
        basis,
        face.subjects.clone(),
        face.relationships.clone(),
        face.properties.clone(),
        text,
        face.actions.clone(),
        face.disclosures.clone(),
    )
    .unwrap()
}

fn assert_negative_identity_and_bound_cases(
    face: &Face,
    plan: &conduit_core::Plan,
    admission: &ManifestationAdmission,
    manifestations: &[Manifestation],
) {
    assert_eq!(
        ManifestationSet::new(
            face,
            vec![manifestations[0].clone(), manifestations[0].clone()],
            plan,
            admission,
        ),
        Err(ManifestationError::DuplicateManifestation)
    );
    assert_eq!(
        ManifestationSet::new(
            face,
            vec![manifestations[0].clone(); conduit_face::MAX_FACE_MANIFESTATIONS + 1],
            plan,
            admission,
        ),
        Err(ManifestationError::TooManyManifestations)
    );
    let mut insufficient_admission = admission.clone();
    insufficient_admission.placement_ids.clear();
    assert_eq!(
        ManifestationSet::new(
            face,
            vec![manifestations[0].clone()],
            plan,
            &insufficient_admission,
        ),
        Err(ManifestationError::UnadmittedManifestation)
    );
    let mut cross_wired = manifestations[0].clone();
    cross_wired.host_id = manifestations[1].host_id.clone();
    assert_eq!(
        cross_wired.validate_against(face, plan),
        Err(ManifestationError::StaleIdentity)
    );
    let mut stale_boot = manifestations[0].clone();
    stale_boot.boot_id = manifestations[1].boot_id.clone();
    assert_eq!(
        stale_boot.validate_against(face, plan),
        Err(ManifestationError::StaleIdentity)
    );
    let mut stale_generation = manifestations[0].clone();
    stale_generation.offer_generation.0 += 1;
    assert_eq!(
        stale_generation.validate_against(face, plan),
        Err(ManifestationError::StaleIdentity)
    );
    let mut wrong_play = manifestations[0].clone();
    wrong_play.active_play_id = manifestations[1].active_play_id.clone();
    assert_eq!(
        wrong_play.validate_against(face, plan),
        Err(ManifestationError::StaleIdentity)
    );
    let mut wrong_capability = manifestations[0].clone();
    wrong_capability.presenter_capability_id = manifestations[1].presenter_capability_id.clone();
    assert_eq!(
        wrong_capability.validate_against(face, plan),
        Err(ManifestationError::StaleIdentity)
    );
    let mut wrong_presenter = manifestations[0].clone();
    wrong_presenter.presenter_implementation_id =
        manifestations[1].presenter_implementation_id.clone();
    assert_eq!(
        wrong_presenter.validate_against(face, plan),
        Err(ManifestationError::StaleIdentity)
    );
    let mut wrong_artifact = manifestations[0].clone();
    wrong_artifact.presenter_artifact_id = manifestations[1].presenter_artifact_id.clone();
    assert_eq!(
        wrong_artifact.validate_against(face, plan),
        Err(ManifestationError::StaleIdentity)
    );
    let mut cross_wired_sign = manifestations[0].clone();
    cross_wired_sign.signs[0].presenter_implementation_id =
        manifestations[1].presenter_implementation_id.clone();
    assert_eq!(
        cross_wired_sign.validate_against(face, plan),
        Err(ManifestationError::InvalidTransition)
    );
    assert!(matches!(
        Manifestation::prepared(
            face,
            plan,
            bind_active_play(
                &plan.plan_id,
                &manifestations[0].host_id,
                &manifestations[0].boot_id,
                manifestations[0].play_sequence,
            ),
            manifestations[0].placement_id.clone(),
            "invented/front".into(),
            "native/display".into(),
            SignId::from("invented/prepared"),
        ),
        Err(ManifestationError::UnknownFaceSubject)
    ));
}
