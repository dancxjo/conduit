use conduit_core::{
    kind_id, BoundedResourceRef, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity,
};
use conduit_human::*;

fn content(profile: &str, bytes: u64) -> BoundedResourceRef {
    BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([1; 32]),
        content_profile: kind_id(profile),
        access_class: ResourceClassId::from("conduit.resource/image-content@1"),
        extent: ResourceExtent {
            bytes,
            items: Some(1),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([2; 32]),
            expires_at: None,
        },
    }
}

#[test]
fn camera_import_and_fixture_materializers_share_one_exact_constructor() {
    let profile = kind_id("media/image-rgba8@1");
    for identity in ["camera", "import", "fixture"] {
        let mut resource = content(profile.as_str(), 4_096);
        resource.access_class = ResourceClassId::from(format!("test/{identity}"));
        ImageObservationReference::new(resource, 640, 480, &profile).unwrap();
    }
}

#[test]
fn resource_profile_dimensions_and_content_bounds_refuse_distinctly() {
    let profile = kind_id("media/image-rgba8@1");
    assert_eq!(
        ImageObservationReference::new(content("media/image-gray8@1", 4_096), 640, 480, &profile,),
        Err(ImageObservationRefusal::WrongProfile)
    );
    assert_eq!(
        ImageObservationReference::new(content(profile.as_str(), 4_096), 0, 480, &profile),
        Err(ImageObservationRefusal::InvalidDimensions)
    );
    assert_eq!(
        ImageObservationReference::new(
            content(profile.as_str(), MAXIMUM_IMAGE_OBSERVATION_BYTES + 1),
            640,
            480,
            &profile,
        ),
        Err(ImageObservationRefusal::ContentTooLarge)
    );
}
