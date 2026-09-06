use conduit_core::{
    kind_id, BoundedResourceRef, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity,
};
use conduit_human::*;

fn image(profile: &str, seed: u8) -> ImageObservationReference {
    ImageObservationReference {
        content: BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest([seed; 32]),
            content_profile: kind_id(profile),
            access_class: ResourceClassId::from("conduit.resource/image-content@1"),
            extent: ResourceExtent {
                bytes: 4_096,
                items: Some(1),
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([seed + 1; 32]),
                expires_at: None,
            },
        },
        width: 640,
        height: 480,
    }
}

#[test]
fn same_composition_accepts_exact_images_from_distinct_realizations() {
    let profile = kind_id("media/image-rgba8@1");
    let imported = compose_image_text(
        &profile,
        image(profile.as_str(), 1),
        "Imported field sample".into(),
        vec![ImageTextMetadata {
            key: "subject".into(),
            value: "north wall".into(),
        }],
    )
    .unwrap();
    let captured = compose_image_text(
        &profile,
        image(profile.as_str(), 3),
        "Captured field sample".into(),
        vec![ImageTextMetadata {
            key: "subject".into(),
            value: "north wall".into(),
        }],
    )
    .unwrap();
    imported.validate(&profile).unwrap();
    captured.validate(&profile).unwrap();
    assert_ne!(
        imported.image.content.identity,
        captured.image.content.identity
    );
    assert_ne!(imported.content_digest, captured.content_digest);
}

#[test]
fn image_type_caption_and_metadata_bounds_refuse_distinctly() {
    let profile = kind_id("media/image-rgba8@1");
    assert_eq!(
        compose_image_text(
            &kind_id("media/image-gray8@1"),
            image(profile.as_str(), 1),
            "caption".into(),
            vec![],
        ),
        Err(ImageTextRefusal::WrongImageProfile)
    );
    assert_eq!(
        compose_image_text(&profile, image(profile.as_str(), 1), String::new(), vec![]),
        Err(ImageTextRefusal::EmptyCaption)
    );
    assert_eq!(
        compose_image_text(
            &profile,
            image(profile.as_str(), 1),
            "x".repeat(MAXIMUM_IMAGE_TEXT_CAPTION_BYTES + 1),
            vec![],
        ),
        Err(ImageTextRefusal::CaptionTooLarge)
    );
    let mut invalid_dimensions = image(profile.as_str(), 1);
    invalid_dimensions.width = 0;
    assert_eq!(
        compose_image_text(&profile, invalid_dimensions, "caption".into(), vec![]),
        Err(ImageTextRefusal::InvalidImageDimensions)
    );
    let mut oversized_image = image(profile.as_str(), 1);
    oversized_image.content.extent.bytes = MAXIMUM_IMAGE_OBSERVATION_BYTES + 1;
    assert_eq!(
        compose_image_text(&profile, oversized_image, "caption".into(), vec![]),
        Err(ImageTextRefusal::ImageTooLarge)
    );
    let duplicate = vec![
        ImageTextMetadata {
            key: "place".into(),
            value: "a".into(),
        },
        ImageTextMetadata {
            key: "place".into(),
            value: "b".into(),
        },
    ];
    assert_eq!(
        compose_image_text(
            &profile,
            image(profile.as_str(), 1),
            "caption".into(),
            duplicate
        ),
        Err(ImageTextRefusal::DuplicateMetadataKey)
    );

    let too_many = (0..=MAXIMUM_IMAGE_TEXT_METADATA_ENTRIES)
        .map(|index| ImageTextMetadata {
            key: format!("key-{index}"),
            value: "value".into(),
        })
        .collect();
    assert_eq!(
        compose_image_text(
            &profile,
            image(profile.as_str(), 1),
            "caption".into(),
            too_many,
        ),
        Err(ImageTextRefusal::TooManyMetadataEntries)
    );
}

#[test]
fn tampering_does_not_validate_as_the_original_composition() {
    let profile = kind_id("media/image-rgba8@1");
    let mut record = compose_image_text(
        &profile,
        image(profile.as_str(), 1),
        "original".into(),
        vec![],
    )
    .unwrap();
    record.caption = "changed".into();
    assert_eq!(
        record.validate(&profile),
        Err(ImageTextRefusal::IntegrityMismatch)
    );
}
