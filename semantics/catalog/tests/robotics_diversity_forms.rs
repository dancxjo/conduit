use conduit_core::{
    BaseImplementationId, BootId, HostAdvertisement, HostId, HostProfileId, OfferGeneration,
    Quantity, QuantityUnit, StructuredInfoValue, StructuredInfoValueShape, StructuredSelection,
    StructuredSelector, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_presentation::{install_geometry_catalogs, robotics_pose2_type};
use conduit_semantic_catalog::{
    deterministic_robotics_structured_fixture, install_robotics_structured_catalogs,
    pose_sample_value, range_sample_value, robotics_motion_request_type, robotics_pose_sample_type,
    robotics_range_observation_type, robotics_twist_interval_type, twist_interval_value,
    RoboticsStructuredRefusal, ROBOTICS_BODY_FRAME,
};

const SOURCE: &str = include_str!("../../../forms/robotics-diversity/main.conduit");

#[test]
fn canonical_form_consumes_structured_observations_and_motion_intent() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_geometry_catalogs(&mut startup, &mut profile).unwrap();
    install_robotics_structured_catalogs(&mut startup, &mut profile).unwrap();
    let parsed = parse_syntax_document(SOURCE);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let checked = check_syntax_document(&parsed, &startup).unwrap();
    let authored =
        expand_canonical_form_for_authoring(&checked, "robotics-diversity", &profile).unwrap();
    assert_eq!(authored.expanded.gears.len(), 2);
    assert_eq!(authored.output_bindings.len(), 5);

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
    assert!(plan.fragments[0]
        .placements
        .iter()
        .all(|placement| placement.authority.is_empty()));
}

#[test]
fn geometry_quantity_sample_and_uncertainty_remain_individually_selectable() {
    let fixture = deterministic_robotics_structured_fixture().unwrap();
    let pose = select(
        &StructuredSelector::field(robotics_pose_sample_type(), "pose").unwrap(),
        &fixture.pose,
    );
    let position = select(
        &StructuredSelector::field(robotics_pose2_type(), "position").unwrap(),
        &pose,
    );
    let x = record_field(&position, "x");
    let StructuredInfoValueShape::Leaf(bytes) = x.shape() else {
        panic!("pose x must be a quantity leaf")
    };
    assert_eq!(
        Quantity::decode(bytes).unwrap(),
        Quantity::new(1_250, QuantityUnit::Millimeter)
    );

    let sample = record_field(&fixture.range, "sample");
    assert_eq!(
        leaf_text(record_field(sample, "source_identity")),
        "sim/front-range"
    );
    let measurement = record_field(&fixture.range, "measurement");
    assert_eq!(
        quantity(record_field(measurement, "uncertainty")),
        Quantity::new(5, QuantityUnit::Millimeter)
    );
}

#[test]
fn unsupported_dimensions_precision_and_frames_refuse_explicitly() {
    assert!(matches!(
        twist_interval_value(
            "map",
            Quantity::new(100, QuantityUnit::Millisecond),
            Quantity::new(1, QuantityUnit::Millimeter),
            Quantity::new(0, QuantityUnit::Millimeter),
            Quantity::new(0, QuantityUnit::Degree),
        ),
        Err(RoboticsStructuredRefusal::UnsupportedFrame { expected, actual })
            if expected == ROBOTICS_BODY_FRAME && actual == "map"
    ));
    assert_eq!(
        twist_interval_value(
            ROBOTICS_BODY_FRAME,
            Quantity::new(0, QuantityUnit::Millisecond),
            Quantity::new(1, QuantityUnit::Millimeter),
            Quantity::new(0, QuantityUnit::Millimeter),
            Quantity::new(0, QuantityUnit::Degree),
        ),
        Err(RoboticsStructuredRefusal::NonPositiveInterval)
    );
    assert!(matches!(
        twist_interval_value(
            ROBOTICS_BODY_FRAME,
            Quantity::new(500, QuantityUnit::Microsecond),
            Quantity::new(1, QuantityUnit::Millimeter),
            Quantity::new(0, QuantityUnit::Millimeter),
            Quantity::new(0, QuantityUnit::Degree),
        ),
        Err(RoboticsStructuredRefusal::InexactPrecision { field: "interval" })
    ));
    assert_eq!(
        twist_interval_value(
            ROBOTICS_BODY_FRAME,
            Quantity::new(60_001, QuantityUnit::Millisecond),
            Quantity::new(1, QuantityUnit::Millimeter),
            Quantity::new(0, QuantityUnit::Millimeter),
            Quantity::new(0, QuantityUnit::Degree),
        ),
        Err(RoboticsStructuredRefusal::OutsideRange { field: "interval" })
    );
    assert!(matches!(
        pose_sample_value(
            "sim/pose",
            1,
            Quantity::new(1, QuantityUnit::Millisecond),
            "map",
            Quantity::new(0, QuantityUnit::Millimeter),
            Quantity::new(0, QuantityUnit::Millimeter),
            Quantity::new(0, QuantityUnit::Degree),
            Quantity::new(-1, QuantityUnit::Millimeter),
            Quantity::new(1, QuantityUnit::Degree),
        ),
        Err(RoboticsStructuredRefusal::NegativeUncertainty {
            field: "position_uncertainty"
        })
    ));
    assert!(matches!(
        range_sample_value(
            "sim/range",
            2,
            Quantity::new(2, QuantityUnit::Millisecond),
            "sensor/front",
            Quantity::new(1_500, QuantityUnit::Micrometer),
            Quantity::new(1, QuantityUnit::Millimeter),
        ),
        Err(RoboticsStructuredRefusal::InexactPrecision { field: "distance" })
    ));
    assert_eq!(
        range_sample_value(
            "sim/range",
            3,
            Quantity::new(3, QuantityUnit::Millisecond),
            "sensor/front",
            Quantity::new(1_000_001, QuantityUnit::Millimeter),
            Quantity::new(1, QuantityUnit::Millimeter),
        ),
        Err(RoboticsStructuredRefusal::OutsideRange { field: "distance" })
    );
}

#[test]
fn physical_motion_authority_is_narrower_than_observation_capability() {
    let observations = robotics_structured_proof_offers();
    assert!(observations
        .iter()
        .all(|offer| offer.authority_requirements.is_empty()));
    let motion = robotics_motion_proof_offer();
    assert_eq!(
        motion.inputs[0].value_kind,
        robotics_motion_request_type()
            .profile()
            .unwrap()
            .value_kind()
            .clone()
    );
    assert_eq!(motion.authority_requirements.len(), 1);
    assert_eq!(
        motion.authority_requirements[0].contract_id.as_str(),
        MOTION_PROOF_AUTHORITY
    );
    assert_eq!(
        motion.authority_requirements[0]
            .host_operation_contract_id
            .as_str(),
        MOTION_PROOF_OPERATION
    );
    assert_eq!(motion.limits.max_active_instances, 1);
    assert_eq!(motion.limits.max_queue_items, 1);
}

#[test]
fn structured_robotics_schema_does_not_leak_robot_protocols() {
    let rendered = format!(
        "{:?}{:?}{:?}",
        robotics_pose_sample_type(),
        robotics_range_observation_type(),
        robotics_twist_interval_type()
    )
    .to_ascii_lowercase();
    for forbidden in ["create-oi", "gpio", "i2c", "spi", "uart", "ros", "packet"] {
        assert!(
            !rendered.contains(forbidden),
            "robotics schema leaked {forbidden}"
        );
    }
}

fn host() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/robotics-structured-proof"),
        boot_id: BootId::from("boot/robotics-structured-proof"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("std/robotics-structured-proof@1"),
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: robotics_structured_proof_offers(),
    }
}

fn select(selector: &StructuredSelector, value: &StructuredInfoValue) -> StructuredInfoValue {
    let StructuredSelection::Matched(value) = selector.select(value).unwrap() else {
        panic!("deterministic robotics selector must match")
    };
    value
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

fn quantity(value: &StructuredInfoValue) -> Quantity {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        panic!("expected quantity")
    };
    Quantity::decode(bytes).unwrap()
}

fn leaf_text(value: &StructuredInfoValue) -> &str {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        panic!("expected text")
    };
    core::str::from_utf8(bytes).unwrap()
}
mod common;

use common::{
    robotics_motion_proof_offer, robotics_structured_proof_offers, MOTION_PROOF_AUTHORITY,
    MOTION_PROOF_OPERATION,
};
