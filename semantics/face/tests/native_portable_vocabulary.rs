use conduit_face::{
    FaceAspect, FaceAspectForm, FaceContributionRole, FaceContributionRoleForm, FaceDepth,
    FaceDepthForm, FacePlace, FacePlaceForm, GenerativeNarratorRole, GenerativeNarratorRoleForm,
    MaskPlanningDisposition, MaskPlanningDispositionForm, MaskWardrobeError, MaskWardrobeErrorForm,
    MaskWardrobeLifetime, MaskWardrobeLifetimeForm, NavigationRefusal, NavigationRefusalForm,
};
use conduit_plot::rust_binding::NativeRustBinding;

#[test]
fn navigation_vocabulary_keeps_its_existing_order_and_exact_codes() {
    assert_eq!(FacePlace::Entrance as u8, 0);
    assert_eq!(FacePlace::Body as u8, 2);
    assert_eq!(FacePlaceForm::encode(FacePlace::Entrance), [0]);
    assert_eq!(FacePlaceForm::decode(&[2]), Ok(FacePlace::Body));

    assert_eq!(FaceAspect::Structure as u8, 0);
    assert_eq!(FaceAspect::Signs as u8, 3);
    assert_eq!(FaceAspectForm::decode(&[2]), Ok(FaceAspect::Play));

    assert!(FaceDepth::Primary < FaceDepth::Exact);
    assert_eq!(FaceDepthForm::encode(FaceDepth::Detail), [2]);
    assert_eq!(
        NavigationRefusalForm::encode(NavigationRefusal::InvalidTruth),
        [7]
    );
}

#[test]
fn wardrobe_face_and_narrator_are_exact_native_vocabulary() {
    assert_eq!(
        MaskWardrobeLifetimeForm::decode(&[1]),
        Ok(MaskWardrobeLifetime::Body)
    );
    assert_eq!(
        MaskPlanningDispositionForm::encode(MaskPlanningDisposition::ReplacementRequired),
        [1]
    );
    assert_eq!(
        MaskWardrobeErrorForm::encode(MaskWardrobeError::StaleSelection),
        [8]
    );
    assert_eq!(
        FaceContributionRoleForm::encode(FaceContributionRole::Transient),
        [3]
    );
    assert_eq!(
        GenerativeNarratorRoleForm::encode(
            GenerativeNarratorRole::TransientFirstPersonBodyNarrator
        ),
        [0]
    );

    let structured = FaceContributionRole::Inspection
        .into_structured()
        .expect("native contribution role");
    assert_eq!(
        FaceContributionRole::from_structured(structured),
        Ok(FaceContributionRole::Inspection)
    );
}
