use conduit_core::{
    kind_id, BoundedResourceRef, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity,
};
use conduit_human::{compose_image_text, ImageObservationReference, ImageTextMetadata};
use conduit_plot::rust_binding::NativeRustBinding;

#[test]
fn image_text_record_round_trips_through_its_native_owner() {
    let profile = kind_id("media/image-rgba8@1");
    let record = compose_image_text(
        &profile,
        ImageObservationReference {
            content: BoundedResourceRef {
                identity: ResourceSemanticIdentity::from_digest([1; 32]),
                content_profile: profile.clone(),
                access_class: ResourceClassId::from("conduit.resource/image-content@1"),
                extent: ResourceExtent {
                    bytes: 4,
                    items: Some(1),
                },
                lifetime: ResourceLifetime {
                    version: ResourceVersionIdentity::from_digest([2; 32]),
                    expires_at: None,
                },
            },
            width: 1,
            height: 1,
        },
        "pixel".into(),
        vec![ImageTextMetadata::new("subject".into(), "wall".into()).unwrap()],
    )
    .unwrap();

    let structured = record.clone().into_structured().unwrap();
    assert_eq!(
        conduit_human::ImageTextRecord::from_structured(structured).unwrap(),
        record
    );
}

#[test]
fn image_text_record_has_no_handwritten_duplicate() {
    let source = include_str!("../src/image_text.rs");
    assert!(!source.contains("pub struct ImageTextRecord"));
    assert!(!source.contains("pub enum ImageTextRefusal"));
}
