use conduit_core::{
    kind_id, BoundedResourceRef, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity,
};
use conduit_form::rust_binding::NativeRustBinding;
use conduit_human::{ImageObservationReference, ImageObservationRefusal};

fn content() -> BoundedResourceRef {
    BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([1; 32]),
        content_profile: kind_id("media/image-rgba8@1"),
        access_class: ResourceClassId::from("conduit.resource/image-content@1"),
        extent: ResourceExtent {
            bytes: 4_096,
            items: Some(1),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([2; 32]),
            expires_at: None,
        },
    }
}

#[test]
fn image_reference_round_trips_at_exact_dimension_bounds() {
    for (width, height) in [(1, 1), (4_096, 4_096)] {
        let reference = ImageObservationReference::new_native(content(), width, height).unwrap();
        let structured = reference.clone().into_structured().unwrap();
        assert_eq!(
            ImageObservationReference::from_structured(structured).unwrap(),
            reference
        );
    }
}

#[test]
fn native_owner_refuses_dimensions_outside_the_portable_contract() {
    assert!(ImageObservationReference::new_native(content(), 0, 480).is_err());
    assert!(ImageObservationReference::new_native(content(), 640, 0).is_err());
    assert!(ImageObservationReference::new_native(content(), 4_097, 480).is_err());
    assert!(ImageObservationReference::new_native(content(), 640, 4_097).is_err());
}

#[test]
fn image_observation_refusals_round_trip_through_the_native_owner() {
    for refusal in [
        ImageObservationRefusal::InvalidResource,
        ImageObservationRefusal::WrongProfile,
        ImageObservationRefusal::InvalidDimensions,
        ImageObservationRefusal::ContentTooLarge,
    ] {
        let structured = refusal.into_structured().unwrap();
        assert_eq!(
            ImageObservationRefusal::from_structured(structured).unwrap(),
            refusal
        );
    }

    assert!(
        !include_str!("../src/image_observation.rs").contains("pub enum ImageObservationRefusal")
    );
}
