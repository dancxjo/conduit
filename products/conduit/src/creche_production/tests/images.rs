//! Canonical Images interaction tests with explicit software Mask ACK.
//! No native renderer, filesystem save, carrier, boot or join proof is claimed.
use super::*;
use crate::creche_images_face::{CrecheImagesFace, ImagesFaceRefusal, ImagesRequest};
use conduit_face::{FaceBasis, FacePropertyValue, MAX_FACE_TOTAL_BYTES};

fn products() -> CrecheProducedImages {
    let mut job = job("release", Arc::new(AtomicBool::new(false)));
    let files = Files::new();
    for host in ["first", "second"] {
        let input = files.input(&job, host, false);
        job.produce_host(host, input, files.output(&format!("{host}.iso")))
            .unwrap();
    }
    // On Linux open files preserve exact retained bytes after fixture-path
    // cleanup. This tests projection custody, not publication or saved paths.
    match job.finish() {
        Ok(products) => products,
        Err((error, _)) => panic!("fixture products: {error:?}"),
    }
}
fn basis(products: &CrecheProducedImages) -> FaceBasis {
    let context = products.context();
    let partition = context
        .plan()
        .plots
        .iter()
        .find(|plot| &plot.plot == context.plot())
        .unwrap();
    FaceBasis {
        body_id: Some(context.plan().body_id.clone()),
        wake_id: Some(context.plan().wake_id.clone()),
        source_document_id: Some(partition.plan.source_document_id.clone()),
        checked_plot_id: Some(partition.plan.checked_plot_id.clone()),
        expanded_plot_id: Some(partition.plan.expanded_plot_id.clone()),
        plan_id: Some(partition.plan.plan_id.clone()),
        active_play_id: Some(context.play().active_play_id.clone()),
        sign_ids: vec![],
    }
}
#[test]
fn images_save_routes_each_exact_receipt_and_done_without_deployment_or_boot_claims() {
    let products = products();
    let images =
        CrecheImagesFace::new(&products, basis(&products), 9, MAX_FACE_TOTAL_BYTES).unwrap();
    let face = images.face();
    face.validate().unwrap();
    let show = mask_fixture::acknowledged_fixture_show(face);
    assert_eq!(face.actions.len(), products.artifacts().len() + 1);
    assert!(face
        .properties
        .iter()
        .any(|property| property.name == "stage" && property.value == FacePropertyValue::Count(4)));
    assert!(face
        .text
        .iter()
        .any(|text| text.text.contains(&products.biography().friendly_name)));
    let mut selected = std::collections::BTreeSet::new();
    for action in &face.actions {
        let returned =
            FaceInteraction::new(face, &show, &action.identity, &action.target, vec![], 1).unwrap();
        match images.request(&show, &returned).unwrap() {
            ImagesRequest::Save {
                host_entry,
                receipt,
            } => {
                assert!(selected.insert(host_entry));
                let artifact = products
                    .artifacts()
                    .iter()
                    .find(|artifact| artifact.receipt().host_entry == host_entry)
                    .unwrap();
                assert_eq!(receipt, artifact.receipt());
                let target_properties = face
                    .properties
                    .iter()
                    .filter(|property| property.subject == action.target)
                    .collect::<Vec<_>>();
                for (name, value) in [
                    ("target", FacePropertyValue::Text(receipt.target.clone())),
                    (
                        "profile-id",
                        FacePropertyValue::Identity(receipt.profile_id.clone()),
                    ),
                    (
                        "artifact-sha256",
                        FacePropertyValue::Text(receipt.artifact_sha256.clone()),
                    ),
                    (
                        "artifact-bytes",
                        FacePropertyValue::Count(receipt.artifact_bytes),
                    ),
                    ("artifact-prepared", FacePropertyValue::Flag(true)),
                    ("boot-observed", FacePropertyValue::Flag(false)),
                    ("join-observed", FacePropertyValue::Flag(false)),
                ] {
                    assert!(
                        target_properties
                            .iter()
                            .any(|property| property.name == name && property.value == value),
                        "{name}"
                    );
                }
            }
            ImagesRequest::Done => assert_eq!(action.name, "Done"),
        }
        assert!(!action.identity.contains("deploy") && !action.identity.contains("boot"));
    }
    assert_eq!(selected.len(), products.artifacts().len());
    assert!(!face
        .properties
        .iter()
        .any(|property| (property.name == "boot-observed"
            || property.name == "join-observed"
            || property.name == "carrier-realized")
            && property.value == FacePropertyValue::Flag(true)));
}
#[test]
fn images_prior_revision_and_stale_show_cannot_route_save() {
    let products = products();
    let previous =
        CrecheImagesFace::new(&products, basis(&products), 9, MAX_FACE_TOTAL_BYTES).unwrap();
    let current =
        CrecheImagesFace::new(&products, basis(&products), 10, MAX_FACE_TOTAL_BYTES).unwrap();
    let old_show = mask_fixture::acknowledged_fixture_show(previous.face());
    let new_show = mask_fixture::acknowledged_fixture_show(current.face());
    let action = &previous.face().actions[0];
    let old_interaction = FaceInteraction::new(
        previous.face(),
        &old_show,
        &action.identity,
        &action.target,
        vec![],
        1,
    )
    .unwrap();
    assert!(matches!(
        current.request(&new_show, &old_interaction),
        Err(ImagesFaceRefusal::StaleInteraction)
    ));
    let action = &current.face().actions[0];
    let current_interaction = FaceInteraction::new(
        current.face(),
        &new_show,
        &action.identity,
        &action.target,
        vec![],
        1,
    )
    .unwrap();
    assert!(matches!(
        current.request(&old_show, &current_interaction),
        Err(ImagesFaceRefusal::StaleInteraction)
    ));
}
#[test]
fn images_refuses_foreign_execution_and_unadmitted_serialized_face_size() {
    let products = products();
    let mut wrong = basis(&products);
    wrong.active_play_id = Some("play/foreign".into());
    assert!(matches!(
        CrecheImagesFace::new(&products, wrong, 9, MAX_FACE_TOTAL_BYTES),
        Err(ImagesFaceRefusal::InvalidBasis)
    ));
    let mut wrong = basis(&products);
    wrong.body_id = Some(
        Body::born(
            "source/foreign".into(),
            "checked/foreign".into(),
            1,
            "sign/foreign".into(),
        )
        .unwrap()
        .body_id,
    );
    assert!(matches!(
        CrecheImagesFace::new(&products, wrong, 9, MAX_FACE_TOTAL_BYTES),
        Err(ImagesFaceRefusal::InvalidBasis)
    ));
    let mut wrong = basis(&products);
    wrong.checked_plot_id = Some("checked/foreign".into());
    assert!(matches!(
        CrecheImagesFace::new(&products, wrong, 9, MAX_FACE_TOTAL_BYTES),
        Err(ImagesFaceRefusal::InvalidBasis)
    ));
    let images =
        CrecheImagesFace::new(&products, basis(&products), 9, MAX_FACE_TOTAL_BYTES).unwrap();
    let bytes = serde_json::to_vec(images.face()).unwrap().len();
    assert!(CrecheImagesFace::new(&products, basis(&products), 9, bytes).is_ok());
    for limit in [0, bytes - 1, MAX_FACE_TOTAL_BYTES + 1] {
        assert!(matches!(
            CrecheImagesFace::new(&products, basis(&products), 9, limit),
            Err(ImagesFaceRefusal::Bounds)
        ));
    }
}
