#![cfg(feature = "form-catalog")]

use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_semantic_catalog::*;

fn composed_record() -> (conduit_core::KindId, conduit_human::ImageTextRecord) {
    use conduit_core::{
        kind_id, BoundedResourceRef, ResourceClassId, ResourceExtent, ResourceLifetime,
        ResourceSemanticIdentity, ResourceVersionIdentity,
    };
    let profile = kind_id("media/image-rgba8@1");
    let image = conduit_human::ImageObservationReference {
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
    let record = conduit_human::compose_image_text(
        &profile,
        image,
        "north wall".into(),
        vec![conduit_human::ImageTextMetadata {
            key: "operator".into(),
            value: "Ada".into(),
        }],
    )
    .unwrap();
    (profile, record)
}

#[test]
fn image_text_composition_is_an_ordinary_browser_neutral_form() {
    let source = include_str!("../../../forms/image-text-compose/main.conduit");
    let lower = source.to_ascii_lowercase();
    for forbidden in [
        "browser",
        "dom",
        "canvas",
        "getusermedia",
        "device",
        "permission",
        "indexeddb",
        "socket",
        "transport",
        "host",
    ] {
        assert!(!lower.contains(forbidden), "Form contains {forbidden}");
    }

    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_human_media_catalogs(&mut startup, &mut profile).unwrap();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authored =
        expand_canonical_form_for_authoring(&checked, "image-text-compose", &profile).unwrap();

    assert_eq!(authored.input_bindings.len(), 2);
    assert_eq!(authored.output_bindings.len(), 1);
    assert_eq!(authored.expanded.gears.len(), 1);
    assert_eq!(
        authored.expanded.gears[0].kind_id.as_str(),
        IMAGE_TEXT_COMPOSE_KIND
    );
    assert_eq!(authored.expanded.connections.len(), 0);
}

#[test]
fn composed_schema_is_finite_and_keeps_image_as_a_resource_reference() {
    use conduit_core::StructuredInfoTypeShape;

    let record = image_text_record_type();
    let StructuredInfoTypeShape::Record { fields, .. } = record.shape() else {
        panic!("image-text result must be a record");
    };
    let image = fields.iter().find(|field| field.name() == "image").unwrap();
    let StructuredInfoTypeShape::Record {
        fields: image_fields,
        ..
    } = image.value_type().shape()
    else {
        panic!("image observation must retain dimensions with its resource")
    };
    let content = image_fields
        .iter()
        .find(|field| field.name() == "content")
        .unwrap();
    assert!(matches!(content.value_type().shape(),
        StructuredInfoTypeShape::Leaf(identity)
            if identity.as_str() == conduit_core::RESOURCE_REFERENCE_INFO_ID));
    let metadata = fields
        .iter()
        .find(|field| field.name() == "metadata")
        .unwrap();
    assert!(matches!(
        metadata.value_type().shape(),
        StructuredInfoTypeShape::Collection { length, .. }
            if usize::from(length) == conduit_human::MAXIMUM_IMAGE_TEXT_METADATA_ENTRIES
    ));
}

#[test]
fn composed_record_round_trips_as_the_exact_structured_port_value() {
    let (profile, record) = composed_record();
    let value = image_text_record_value(&record, &profile).unwrap();
    assert_eq!(value.value_type(), &image_text_record_type());
    assert_eq!(
        image_text_record_from_value(&value, &profile).unwrap(),
        record
    );
}

#[test]
fn structured_port_decode_keeps_profile_and_integrity_refusals_exact() {
    let (profile, record) = composed_record();
    let value = image_text_record_value(&record, &profile).unwrap();
    assert_eq!(
        image_text_record_from_value(&value, &conduit_core::kind_id("media/image-gray8@1")),
        Err(ImageTextValueRefusal::InvalidRecord(
            conduit_human::ImageTextRefusal::WrongImageProfile
        ))
    );
}

#[test]
fn delivery_record_is_an_ordinary_composition_over_shared_framing() {
    let source = include_str!("../../../forms/image-text-delivery-record/main.conduit");
    let lower = source.to_ascii_lowercase();
    for forbidden in [
        "websocket",
        "webrtc",
        "browser",
        "transport",
        "socket",
        "host",
        "dom",
    ] {
        assert!(!lower.contains(forbidden), "Form contains {forbidden}");
    }
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_net::install_typed_record_catalogs(&mut startup, &mut profile).unwrap();
    install_human_media_catalogs(&mut startup, &mut profile).unwrap();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authored =
        expand_canonical_form_for_authoring(&checked, "image-text-delivery-record", &profile)
            .unwrap();
    assert_eq!(authored.expanded.gears.len(), 3);
    assert!(authored
        .expanded
        .gears
        .iter()
        .any(|gear| gear.kind_id.as_str() == conduit_net::TYPED_RECORD_FRAME_KIND));
    assert_eq!(authored.expanded.connections.len(), 2);

    let (image_profile, record) = composed_record();
    let typed = image_text_typed_record_value(&record, &image_profile).unwrap();
    let restored = conduit_net::value_from_typed_record(&typed).unwrap();
    assert_eq!(
        image_text_record_from_value(&restored, &image_profile).unwrap(),
        record
    );
}
