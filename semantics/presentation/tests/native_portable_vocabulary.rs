use conduit_plot::rust_binding::NativeRustBinding;
use conduit_presentation::{
    FaceContributionRole, FaceContributionRoleForm, GenerativeNarratorRole,
    GenerativeNarratorRoleForm, MaskPlanningDisposition, MaskPlanningDispositionForm,
    MaskWardrobeError, MaskWardrobeErrorForm, MaskWardrobeLifetime, MaskWardrobeLifetimeForm,
    NavigationRefusal, NavigationRefusalForm, PresentationAspect, PresentationAspectForm,
    PresentationDepth, PresentationDepthForm, PresentationPlace, PresentationPlaceForm,
};

#[test]
fn navigation_vocabulary_keeps_its_existing_order_and_exact_codes() {
    assert_eq!(PresentationPlace::Entrance as u8, 0);
    assert_eq!(PresentationPlace::Body as u8, 2);
    assert_eq!(
        PresentationPlaceForm::encode(PresentationPlace::Entrance),
        [0]
    );
    assert_eq!(
        PresentationPlaceForm::decode(&[2]),
        Ok(PresentationPlace::Body)
    );

    assert_eq!(PresentationAspect::Structure as u8, 0);
    assert_eq!(PresentationAspect::Signs as u8, 3);
    assert_eq!(
        PresentationAspectForm::decode(&[2]),
        Ok(PresentationAspect::Play)
    );

    assert!(PresentationDepth::Primary < PresentationDepth::Exact);
    assert_eq!(
        PresentationDepthForm::encode(PresentationDepth::Detail),
        [2]
    );
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
