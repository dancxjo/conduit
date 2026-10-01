use conduit_core::{
    kind_id, BoundedResourceRef, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity,
};
use conduit_human::*;

fn record() -> (conduit_core::KindId, ImageTextRecord) {
    let profile = kind_id("media/image-rgba8@1");
    let image = ImageObservationReference {
        content: BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest([1; 32]),
            content_profile: profile.clone(),
            access_class: ResourceClassId::from("conduit.resource/image-content@1"),
            extent: ResourceExtent {
                bytes: 4_096,
                items: Some(1),
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([2; 32]),
                expires_at: None,
            },
        },
        width: 640,
        height: 480,
    };
    let record = compose_image_text(
        &profile,
        image,
        "portable caption".into(),
        vec![ImageTextMetadata::new("subject".into(), "north wall".into()).unwrap()],
    )
    .unwrap();
    assert_eq!(
        record.content_digest,
        [
            230, 35, 97, 179, 53, 152, 137, 190, 104, 187, 56, 23, 64, 169, 10, 23, 97, 188, 88,
            152, 215, 44, 179, 188, 206, 155, 20, 152, 254, 71, 14, 222,
        ]
    );
    (profile, record)
}

#[test]
fn caller_buffer_round_trip_preserves_exact_record() {
    let (profile, record) = record();
    let mut buffer = [0; MAXIMUM_IMAGE_TEXT_ENCODED_BYTES];
    let written = record.encode_into(&profile, &mut buffer).unwrap();
    let decoded = ImageTextRecord::decode(&profile, &buffer[..written]).unwrap();
    assert_eq!(decoded, record);
    assert!(written < buffer.len());
}

#[test]
fn encoding_refuses_unadmitted_output_and_trailing_input() {
    let (profile, record) = record();
    assert_eq!(
        record.encode_into(&profile, &mut [0; 8]),
        Err(ImageTextCodecRefusal::OutputTooSmall)
    );

    let mut buffer = [0; MAXIMUM_IMAGE_TEXT_ENCODED_BYTES];
    let written = record.encode_into(&profile, &mut buffer).unwrap();
    buffer[written] = 7;
    assert_eq!(
        ImageTextRecord::decode(&profile, &buffer[..=written]),
        Err(ImageTextCodecRefusal::Malformed)
    );
}

#[test]
fn decoding_refuses_wrong_version_profile_and_integrity() {
    let (profile, record) = record();
    let mut buffer = [0; MAXIMUM_IMAGE_TEXT_ENCODED_BYTES];
    let written = record.encode_into(&profile, &mut buffer).unwrap();

    buffer[0] = IMAGE_TEXT_ENCODING_VERSION + 1;
    assert_eq!(
        ImageTextRecord::decode(&profile, &buffer[..written]),
        Err(ImageTextCodecRefusal::UnsupportedVersion)
    );
    buffer[0] = IMAGE_TEXT_ENCODING_VERSION;
    assert_eq!(
        ImageTextRecord::decode(&kind_id("media/image-gray8@1"), &buffer[..written]),
        Err(ImageTextCodecRefusal::InvalidRecord(
            ImageTextRefusal::WrongImageProfile
        ))
    );
    buffer[written - 1] ^= 1;
    assert_eq!(
        ImageTextRecord::decode(&profile, &buffer[..written]),
        Err(ImageTextCodecRefusal::InvalidRecord(
            ImageTextRefusal::IntegrityMismatch
        ))
    );
}
