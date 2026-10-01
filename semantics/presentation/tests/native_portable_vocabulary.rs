use conduit_form::rust_binding::NativeRustBinding;
use conduit_presentation::{
    FaceContributionRole, FaceContributionRoleCode, GenerativeNarratorRole,
    GenerativeNarratorRoleCode, MaskPlanningDisposition, MaskPlanningDispositionCode,
    MaskWardrobeError, MaskWardrobeErrorCode, MaskWardrobeLifetime, MaskWardrobeLifetimeCode,
    NavigationRefusal, NavigationRefusalCode, PresentationAspect, PresentationAspectCode,
    PresentationDepth, PresentationDepthCode, PresentationPlace, PresentationPlaceCode,
};

#[test]
fn navigation_vocabulary_keeps_its_existing_order_and_exact_codes() {
    assert_eq!(PresentationPlace::Entrance as u8, 0);
    assert_eq!(PresentationPlace::Body as u8, 2);
    assert_eq!(
        PresentationPlaceCode::encode(PresentationPlace::Entrance),
        [0]
    );
    assert_eq!(
        PresentationPlaceCode::decode(&[2]),
        Ok(PresentationPlace::Body)
    );

    assert_eq!(PresentationAspect::Structure as u8, 0);
    assert_eq!(PresentationAspect::Signs as u8, 3);
    assert_eq!(
        PresentationAspectCode::decode(&[2]),
        Ok(PresentationAspect::Play)
    );

    assert!(PresentationDepth::Primary < PresentationDepth::Exact);
    assert_eq!(
        PresentationDepthCode::encode(PresentationDepth::Detail),
        [2]
    );
    assert_eq!(
        NavigationRefusalCode::encode(NavigationRefusal::InvalidTruth),
        [7]
    );
}

#[test]
fn wardrobe_face_and_narrator_are_exact_native_vocabulary() {
    assert_eq!(
        MaskWardrobeLifetimeCode::decode(&[1]),
        Ok(MaskWardrobeLifetime::Body)
    );
    assert_eq!(
        MaskPlanningDispositionCode::encode(MaskPlanningDisposition::ReplacementRequired),
        [1]
    );
    assert_eq!(
        MaskWardrobeErrorCode::encode(MaskWardrobeError::StaleSelection),
        [8]
    );
    assert_eq!(
        FaceContributionRoleCode::encode(FaceContributionRole::Transient),
        [3]
    );
    assert_eq!(
        GenerativeNarratorRoleCode::encode(
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
