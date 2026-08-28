use conduit_core::{
    BaseImplementationId, BootId, BoundedResourceRef, HostAdvertisement, HostId, HostProfileId,
    OfferGeneration, Quantity, QuantityUnit, StructuredInfoTypeShape, StructuredInfoValue,
    StructuredInfoValueShape, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_presentation::{install_geometry_catalogs, point2_type, rect2_type};
use conduit_semantic_catalog::{
    deterministic_detect_image, deterministic_vision_fixture, image_resource_type,
    install_vision_catalogs, validate_confidence, vision_detection_type, vision_detections_type,
    vision_keypoint_type, VisionRefusal, MAXIMUM_VISION_DETECTIONS, MAXIMUM_VISION_LANDMARKS,
    VISION_DETECT_KIND, VISION_FIXTURE_KIND, VISION_IMAGE_ACCESS_CLASS,
    VISION_IMAGE_CONTENT_PROFILE,
};

const SOURCE: &str = include_str!("../../../examples/vision-metadata.conduit");

#[test]
fn image_resource_and_detection_metadata_flow_through_one_ordinary_form() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_geometry_catalogs(&mut startup, &mut profile).unwrap();
    install_vision_catalogs(&mut startup, &mut profile).unwrap();
    let parsed = parse_syntax_document(SOURCE);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let checked = check_syntax_document(&parsed, &startup).unwrap();
    let authored =
        expand_canonical_form_for_authoring(&checked, "vision-metadata", &profile).unwrap();
    assert_eq!(authored.expanded.gears.len(), 2);
    assert_eq!(authored.output_bindings.len(), 2);

    let host = host();
    let placements = conduit_planner::default_expanded_placements(
        &authored.expanded,
        core::slice::from_ref(&host),
    )
    .unwrap();
    let plan = conduit_planner::plan_expanded_canonical(
        &authored.expanded,
        &[host],
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
    )
    .unwrap();
    for kind in [VISION_FIXTURE_KIND, VISION_DETECT_KIND] {
        let placement = plan.fragments[0]
            .placements
            .iter()
            .find(|placement| placement.kind_id.as_str() == kind)
            .unwrap();
        assert_eq!(
            placement.host_operations[0].contract_id.as_str(),
            DOMAIN_PROOF_OPERATION
        );
        assert!(placement.resources.is_empty());
        assert!(placement.authority.is_empty());
    }
}

#[test]
fn deterministic_detector_emits_two_bounded_model_derived_detections() {
    let fixture = deterministic_vision_fixture().unwrap();
    let reference =
        BoundedResourceRef::decode(leaf_bytes(record_field(&fixture.image, "content"))).unwrap();
    assert_eq!(
        reference.content_profile.as_str(),
        VISION_IMAGE_CONTENT_PROFILE
    );
    assert_eq!(reference.access_class.as_str(), VISION_IMAGE_ACCESS_CLASS);
    assert_eq!(reference.extent.bytes, 12_288);

    let batch = deterministic_detect_image(&fixture.image).unwrap();
    assert_eq!(batch.value_type(), &vision_detections_type());
    let slots = collection(&batch);
    assert_eq!(slots.len(), usize::from(MAXIMUM_VISION_DETECTIONS));
    assert_eq!(variant_tag(&slots[0]), "detection");
    assert_eq!(variant_tag(&slots[1]), "detection");
    assert_eq!(variant_tag(&slots[2]), "unused");
    assert_eq!(variant_tag(&slots[3]), "unused");

    let first = variant_payload(&slots[0]);
    assert_eq!(leaf_text(record_field(first, "classification")), "square");
    let provenance = record_field(first, "provenance");
    assert_eq!(
        variant_tag(record_field(provenance, "evidence_class")),
        "model_derived"
    );
    assert_eq!(
        leaf_text(record_field(provenance, "source")),
        "fixture/shape-detector"
    );
    let landmarks = collection(record_field(first, "landmarks"));
    assert_eq!(landmarks.len(), usize::from(MAXIMUM_VISION_LANDMARKS));
    assert_eq!(variant_tag(&landmarks[0]), "keypoint");
    assert_eq!(variant_tag(&landmarks[2]), "unused");
}

#[test]
fn regions_and_landmarks_reuse_nominal_geometry_types() {
    let detection = vision_detection_type();
    let StructuredInfoTypeShape::Record { fields, .. } = detection.shape() else {
        panic!("detection must be a record")
    };
    assert_eq!(
        fields
            .iter()
            .find(|field| field.name() == "region")
            .unwrap()
            .value_type(),
        &rect2_type()
    );
    let keypoint = vision_keypoint_type();
    let StructuredInfoTypeShape::Record { fields, .. } = keypoint.shape() else {
        panic!("keypoint must be a record")
    };
    assert_eq!(
        fields
            .iter()
            .find(|field| field.name() == "point")
            .unwrap()
            .value_type(),
        &point2_type()
    );
}

#[test]
fn confidence_and_pixel_storage_refuse_semantic_shortcuts() {
    assert_eq!(
        validate_confidence(Quantity::new(1, QuantityUnit::Meter)),
        Err(VisionRefusal::NonRatioConfidence)
    );
    assert_eq!(
        validate_confidence(Quantity::new(101, QuantityUnit::Percent)),
        Err(VisionRefusal::ConfidenceOutOfRange)
    );
    assert_eq!(
        validate_confidence(Quantity::new(875_000, QuantityUnit::Millionth)),
        Ok(())
    );

    let rendered = format!("{:?}", image_resource_type()).to_ascii_lowercase();
    assert!(rendered.contains(conduit_core::RESOURCE_REFERENCE_INFO_ID));
    for forbidden in [
        "value/bytes",
        "base64",
        "json",
        "pixel-buffer",
        "camera-api",
    ] {
        assert!(
            !rendered.contains(forbidden),
            "image schema leaked {forbidden}"
        );
    }
}

fn host() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/vision-proof"),
        boot_id: BootId::from("boot/vision-proof"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("std/vision-proof@1"),
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: vision_proof_offers(),
    }
}

fn record_field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("expected record")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value()
}

fn collection(value: &StructuredInfoValue) -> &[StructuredInfoValue] {
    let StructuredInfoValueShape::Collection(values) = value.shape() else {
        panic!("expected collection")
    };
    values
}

fn variant_tag(value: &StructuredInfoValue) -> &str {
    let StructuredInfoValueShape::Variant { tag, .. } = value.shape() else {
        panic!("expected variant")
    };
    tag
}

fn variant_payload(value: &StructuredInfoValue) -> &StructuredInfoValue {
    let StructuredInfoValueShape::Variant { payload, .. } = value.shape() else {
        panic!("expected variant")
    };
    payload
}

fn leaf_text(value: &StructuredInfoValue) -> &str {
    core::str::from_utf8(leaf_bytes(value)).unwrap()
}

fn leaf_bytes(value: &StructuredInfoValue) -> &[u8] {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        panic!("expected leaf")
    };
    bytes
}
mod common;

use common::{vision_proof_offers, DOMAIN_PROOF_OPERATION};
