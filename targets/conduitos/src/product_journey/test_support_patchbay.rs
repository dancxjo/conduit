//! Genuine CPU Mask acknowledgement for native owner/lifecycle regression tests.
use super::*;
use conduit_face::{FaceInteraction, FaceInteractionArgument};

pub(crate) fn submit_patchbay(
    journey: &mut ProductJourney,
    action_id: &str,
    subject: Option<&str>,
) {
    let projection = journey.projection();
    let mut door = crate::front_door::FrontDoor::new(
        projection.host_id.clone(),
        projection.boot_id.clone(),
        projection.offer_generation,
        "profile",
        "build",
        "image",
        projection.source_document_id.clone().unwrap(),
        projection.checked_plot_id.clone().unwrap(),
        7,
        true,
    );
    door.observe_product(journey).unwrap();
    let face = door.face().unwrap();
    let provider = crate::product_bases::fixture_surface_provider();
    let mut mask = crate::native_face_mask::NativeFaceMask::prepare(
        projection.host_id,
        projection.boot_id,
        projection.offer_generation,
        "build",
        provider.entry.base_id.clone(),
        "surface/native-owner-regression",
        &provider,
    )
    .unwrap();
    let mut display = crate::native_face_mask::tests::CountingDisplay::default();
    mask.present(
        face.clone(),
        journey.revision(),
        journey.revision(),
        &mut display,
    )
    .unwrap();
    let show = mask.show().unwrap().clone();
    let action = face
        .actions
        .iter()
        .find(|action| action.identity == action_id)
        .unwrap();
    let arguments = subject.map_or_else(Vec::new, |subject| {
        alloc::vec![FaceInteractionArgument {
            name: "subject".into(),
            value_kind: action.arguments[0].contract.value_kind.as_str().into(),
            value: subject.as_bytes().to_vec()
        }]
    });
    let input =
        FaceInteraction::new(&face, &show, &action.identity, &action.target, arguments, 1).unwrap();
    let returned = mask.submit(input).unwrap();
    assert_eq!(
        journey
            .accept_patchbay_interaction(face, show, &returned.interaction)
            .unwrap(),
        None
    );
}
