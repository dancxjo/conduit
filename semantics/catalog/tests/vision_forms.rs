use conduit_core::{
    process_owned_line_offer_with_limits, ArtifactId, BaseImplementationId, BootId,
    BoundedResourceRef, CapabilityId, ExecutionProfileId, HostAdvertisement, HostId, HostProfileId,
    ImplementationId, LinkLimits, OfferGeneration, Quantity, QuantityUnit, StructuredInfoTypeShape,
    StructuredInfoValue, StructuredInfoValueShape, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_planner::{
    plan_expanded_canonical_with_options, PlacementChoice, PlacementChoices, PlanningOptions,
};
use conduit_presentation::{install_geometry_catalogs, point2_type, rect2_type};
use conduit_semantic_catalog::{
    deterministic_detect_image, deterministic_vision_fixture, image_resource_type,
    install_vision_catalogs, validate_confidence, vision_detection_type, vision_detections_type,
    vision_keypoint_type, VisionRefusal, MAXIMUM_VISION_DETECTIONS, MAXIMUM_VISION_LANDMARKS,
    VISION_DETECT_KIND, VISION_FIXTURE_KIND, VISION_IMAGE_ACCESS_CLASS,
    VISION_IMAGE_CONTENT_PROFILE,
};
use std::collections::BTreeMap;

const SOURCE: &str = include_str!("../../../forms/vision-metadata/main.conduit");
const CONTINUOUS_SOURCE: &str = include_str!("../../../forms/vision/main.conduit");

#[test]
fn continuous_vision_is_checked_bounded_and_provider_neutral() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_geometry_catalogs(&mut startup, &mut profile).unwrap();
    install_vision_catalogs(&mut startup, &mut profile).unwrap();

    let parsed = parse_syntax_document(CONTINUOUS_SOURCE);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let checked = check_syntax_document(&parsed, &startup).unwrap();
    let authored = expand_canonical_form_for_authoring(&checked, "vision", &profile).unwrap();
    assert_eq!(authored.expanded.gears.len(), 8);

    let kinds = authored
        .expanded
        .gears
        .iter()
        .map(|gear| gear.kind_id.as_str())
        .collect::<Vec<_>>();
    for expected in [
        conduit_semantic_catalog::FLOW_COALESCE_LATEST_KIND,
        conduit_semantic_catalog::VISION_MOTION_KIND,
        conduit_semantic_catalog::VISION_OBJECTS_KIND,
        conduit_semantic_catalog::VISION_OCR_KIND,
        conduit_semantic_catalog::VISION_TRACK_KIND,
        conduit_semantic_catalog::VISION_DESCRIBE_KIND,
        conduit_semantic_catalog::VISION_EXPERIENCE_KIND,
    ] {
        assert!(kinds.contains(&expected), "missing {expected}");
    }
    assert!(!CONTINUOUS_SOURCE.contains("camera"));
    assert!(!CONTINUOUS_SOURCE.contains("opencv"));

    let flow = conduit_semantic_catalog::flow_coalesce_latest_contract(
        image_resource_type().profile().unwrap().value_kind(),
        conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    );
    assert_eq!(flow.limits.max_queue_items, 1);
    assert_eq!(
        conduit_semantic_catalog::reviewed_flow_pressure_policy(&flow.kind_id),
        Some(conduit_core::DeliveryPressurePolicy::CoalesceLatest)
    );
}

#[test]
fn continuous_vision_seals_the_same_authored_graph_across_two_hosts() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_geometry_catalogs(&mut startup, &mut profile).unwrap();
    install_vision_catalogs(&mut startup, &mut profile).unwrap();
    let checked =
        check_syntax_document(&parse_syntax_document(CONTINUOUS_SOURCE), &startup).unwrap();
    let authored = expand_canonical_form_for_authoring(&checked, "vision", &profile).unwrap();

    let flow_contract = conduit_semantic_catalog::flow_coalesce_latest_contract(
        image_resource_type().profile().unwrap().value_kind(),
        conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    );
    let flow_offer = proof_domain_offer(
        flow_contract.kind_id,
        flow_contract.inputs,
        flow_contract.outputs,
        conduit_semantic_catalog::FLOW_COALESCE_LATEST_REVISION,
        DOMAIN_PROOF_OPERATION,
    );
    let mut edge = host();
    edge.host_id = HostId::from("host/vision-edge");
    edge.boot_id = BootId::from("boot/vision-edge");
    edge.capabilities.push(flow_offer);
    let mut model = host();
    model.host_id = HostId::from("host/vision-model");
    model.boot_id = BootId::from("boot/vision-model");

    let placements = PlacementChoices {
        by_gear: authored
            .expanded
            .gears
            .iter()
            .map(|gear| {
                let selected = if matches!(
                    gear.kind_id.as_str(),
                    conduit_semantic_catalog::VISION_DESCRIBE_KIND
                        | conduit_semantic_catalog::VISION_EXPERIENCE_KIND
                ) {
                    &model
                } else {
                    &edge
                };
                let offer = selected
                    .capabilities
                    .iter()
                    .find(|offer| offer.kind_id == gear.kind_id)
                    .unwrap();
                (
                    gear.gear_id.clone(),
                    PlacementChoice {
                        host_id: selected.host_id.clone(),
                        capability_id: offer.capability_id.clone(),
                    },
                )
            })
            .collect(),
    };
    let maximum = conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32;
    let line = process_owned_line_offer_with_limits(
        "line/vision-edge-model",
        "binding/vision-edge-model",
        BaseImplementationId::from("conduit.proof/vision-line@1"),
        "fixture/vision-line",
        &edge,
        &model,
        LinkLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: maximum,
            maximum_buffered_bytes: maximum,
            maximum_frame_bytes: maximum + 256,
        },
    );
    let plan = plan_expanded_canonical_with_options(
        &authored.expanded,
        &[edge, model],
        &placements,
        &[
            BaseImplementationId::from("conduit.base/local@1"),
            BaseImplementationId::from("conduit.proof/vision-line@1"),
        ],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: maximum,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[line],
        },
    )
    .unwrap();
    assert_eq!(plan.fragments.len(), 2);
    assert!(plan
        .fragments
        .iter()
        .any(|fragment| fragment.host_id == HostId::from("host/vision-edge")));
    assert!(plan
        .fragments
        .iter()
        .any(|fragment| fragment.host_id == HostId::from("host/vision-model")));
    assert!(plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .any(|connection| connection
            .admitted_lines
            .iter()
            .any(|line| { line.line_id.as_str() == "line/vision-edge-model" })));
}

#[test]
fn model_provider_loss_refuses_and_a_compatible_replacement_changes_only_realization() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_geometry_catalogs(&mut startup, &mut profile).unwrap();
    install_vision_catalogs(&mut startup, &mut profile).unwrap();
    let checked =
        check_syntax_document(&parse_syntax_document(CONTINUOUS_SOURCE), &startup).unwrap();
    let authored = expand_canonical_form_for_authoring(&checked, "vision", &profile).unwrap();

    let mut edge = host();
    edge.host_id = HostId::from("host/vision-edge");
    edge.boot_id = BootId::from("boot/vision-edge");
    edge.capabilities
        .retain(|offer| offer.kind_id.as_str() != conduit_semantic_catalog::VISION_DESCRIBE_KIND);
    let flow_contract = conduit_semantic_catalog::flow_coalesce_latest_contract(
        image_resource_type().profile().unwrap().value_kind(),
        conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    );
    edge.capabilities.push(proof_domain_offer(
        flow_contract.kind_id,
        flow_contract.inputs,
        flow_contract.outputs,
        conduit_semantic_catalog::FLOW_COALESCE_LATEST_REVISION,
        DOMAIN_PROOF_OPERATION,
    ));
    let mut model = host();
    model.host_id = HostId::from("host/vision-model-a");
    model.boot_id = BootId::from("boot/vision-model-a");
    model
        .capabilities
        .retain(|offer| offer.kind_id.as_str() == conduit_semantic_catalog::VISION_DESCRIBE_KIND);

    let original = conduit_planner::default_expanded_placements(
        &authored.expanded,
        &[edge.clone(), model.clone()],
    )
    .unwrap();
    let describe = authored
        .expanded
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == conduit_semantic_catalog::VISION_DESCRIBE_KIND)
        .unwrap();
    let original_choice = &original.by_gear[&describe.gear_id];
    assert_eq!(original_choice.host_id, model.host_id);

    assert!(
        conduit_planner::default_expanded_placements(&authored.expanded, &[edge.clone()]).is_err(),
        "provider loss must refuse rather than hide a fallback"
    );

    let mut replacement = model.clone();
    replacement.host_id = HostId::from("host/vision-model-b");
    replacement.boot_id = BootId::from("boot/vision-model-b");
    let replacement_offer = &mut replacement.capabilities[0];
    replacement_offer.capability_id = CapabilityId::from("replacement/vision-model-describe@1");
    replacement_offer.implementation.execution_profile_id =
        ExecutionProfileId::from("replacement/vision-model-profile@1");
    replacement_offer.implementation.implementation_id =
        ImplementationId::from("replacement/vision-model-describe@1");
    replacement_offer.implementation.artifact_id =
        ArtifactId::from("replacement/vision-model-artifact@1");

    let replanned = conduit_planner::default_expanded_placements(
        &authored.expanded,
        &[edge, replacement.clone()],
    )
    .unwrap();
    let replacement_choice = &replanned.by_gear[&describe.gear_id];
    assert_eq!(replacement_choice.host_id, replacement.host_id);
    assert_ne!(replacement_choice, original_choice);
    assert_eq!(
        replacement.capabilities[0].kind_id,
        model.capabilities[0].kind_id
    );
    assert_eq!(
        replacement.capabilities[0].kind_contract_revision,
        model.capabilities[0].kind_contract_revision
    );
}

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
            placement.host_calls[0].contract_id.as_str(),
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
        bases: vec![],
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

use common::{proof_domain_offer, vision_proof_offers, DOMAIN_PROOF_OPERATION};
