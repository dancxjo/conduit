use conduit_core::{
    BaseImplementationId, BootId, ConfigurationValue, HostAdvertisement, HostId, HostProfileId,
    OfferGeneration, Quantity, StructuredInfoTypeShape, StructuredInfoValue,
    StructuredInfoValueShape, Unit, PROTOCOL_VERSION,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_presentation::{
    apply_transform2, apply_transform2_to_path, image_region2_type, install_geometry_catalogs,
    path2_type, path2_value, point2_type, point2_value, rect2_type, robotics_pose2_type,
    transform2_value, GeometryRefusal, APPLY_TRANSFORM2_KIND, MAXIMUM_GEOMETRY_PATH_POINTS,
    POINT2_LITERAL_KIND,
};
use conduit_std_host::hosted_geometry::{geometry_std_offers, GEOMETRY_HOST_CALL};

const SOURCE: &str = include_str!("../../../plots/geometry-spatial/main.conduit");

#[test]
fn canonical_geometry_configuration_flows_through_one_checked_plot_and_plan() {
    let (startup, profile) = catalogs();
    let syntax = parse_syntax_document(SOURCE);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, "geometry-spatial", &profile).unwrap();
    assert_eq!(authored.expanded.gears.len(), 2);
    assert_eq!(authored.output_bindings.len(), 1);
    assert!(authored
        .expanded
        .gears
        .iter()
        .any(|gear| gear.kind_id.as_str() == POINT2_LITERAL_KIND));
    assert!(authored
        .expanded
        .gears
        .iter()
        .any(|gear| gear.kind_id.as_str() == APPLY_TRANSFORM2_KIND));
    for gear in &authored.expanded.gears {
        let ConfigurationValue::Structured(value) = &gear.configuration[0].value else {
            panic!("geometry configuration must remain canonical structured Info")
        };
        StructuredInfoValue::from_canonical_bytes(value.canonical_value()).unwrap();
    }

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
    assert_eq!(plan.fragments[0].placements.len(), 2);
    let transform = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == APPLY_TRANSFORM2_KIND)
        .unwrap();
    assert_eq!(
        transform.host_calls[0].contract_id.as_str(),
        GEOMETRY_HOST_CALL
    );
}

#[test]
fn exact_translation_reports_frame_unit_and_overflow_refusals() {
    let point = point2_value(
        "robot/base",
        Quantity::new(1000, Unit::Millimeter),
        Quantity::new(2000, Unit::Millimeter),
    )
    .unwrap();
    let transform = transform2_value(
        "robot/base",
        "map",
        Quantity::new(1, Unit::Meter),
        Quantity::new(-500, Unit::Millimeter),
    )
    .unwrap();
    let moved = apply_transform2(&point, &transform).unwrap();
    assert_eq!(point_quantities(&moved), (2000, 1500));
    assert_eq!(point_frame(&moved), "map");

    let wrong_frame = transform2_value(
        "camera",
        "map",
        Quantity::new(0, Unit::Millimeter),
        Quantity::new(0, Unit::Millimeter),
    )
    .unwrap();
    assert_eq!(
        apply_transform2(&point, &wrong_frame),
        Err(GeometryRefusal::FrameMismatch {
            expected: "camera".into(),
            actual: "robot/base".into(),
        })
    );

    let meter_point = point2_value(
        "robot/base",
        Quantity::new(1, Unit::Meter),
        Quantity::new(2, Unit::Meter),
    )
    .unwrap();
    let fractional_translation = transform2_value(
        "robot/base",
        "map",
        Quantity::new(1, Unit::Millimeter),
        Quantity::new(0, Unit::Meter),
    )
    .unwrap();
    let fractional = apply_transform2(&meter_point, &fractional_translation).unwrap();
    let StructuredInfoValueShape::Record(fields) = fractional.shape() else {
        panic!("Point2 is a record");
    };
    let x = fields.iter().find(|field| field.name() == "x").unwrap();
    let StructuredInfoValueShape::Leaf(bytes) = x.value().shape() else {
        panic!("x is a quantity leaf");
    };
    assert_eq!(
        Quantity::decode(bytes).unwrap(),
        Quantity::from_decimal(1001, -3, Unit::Meter).unwrap()
    );
    assert_eq!(
        transform2_value(
            "robot/base",
            "map",
            Quantity::new(1, Unit::Degree),
            Quantity::new(0, Unit::Meter),
        ),
        Err(GeometryRefusal::IncompatibleUnit)
    );

    let overflow_point = point2_value(
        "robot/base",
        Quantity::from_decimal(
            99_999_999_999_999_999_999_999_999_999_999_999_999,
            0,
            Unit::Millimeter,
        )
        .unwrap(),
        Quantity::new(0, Unit::Millimeter),
    )
    .unwrap();
    let overflow = transform2_value(
        "robot/base",
        "map",
        Quantity::new(2, Unit::Millimeter),
        Quantity::new(0, Unit::Millimeter),
    )
    .unwrap();
    assert_eq!(
        apply_transform2(&overflow_point, &overflow),
        Err(GeometryRefusal::Overflow)
    );
}

#[test]
fn path_operations_have_a_reviewed_hard_bound() {
    assert_eq!(
        path2_type(MAXIMUM_GEOMETRY_PATH_POINTS + 1),
        Err(GeometryRefusal::TooManyPoints)
    );
    assert_eq!(path2_type(0), Err(GeometryRefusal::TooManyPoints));

    let points = (0..4)
        .map(|x| {
            point2_value(
                "robot/base",
                Quantity::new(x, Unit::Millimeter),
                Quantity::new(0, Unit::Millimeter),
            )
            .unwrap()
        })
        .collect();
    let path = path2_value(points).unwrap();
    let transform = transform2_value(
        "robot/base",
        "map",
        Quantity::new(10, Unit::Millimeter),
        Quantity::new(20, Unit::Millimeter),
    )
    .unwrap();
    let moved = apply_transform2_to_path(&path, &transform).unwrap();
    let points = record_value(&moved, "points");
    let StructuredInfoValueShape::Collection(points) = points.shape() else {
        panic!("path points must remain one exact finite collection")
    };
    assert_eq!(points.len(), 4);
    assert!(points.iter().all(|point| point_frame(point) == "map"));
}

#[test]
fn robotics_and_vision_reuse_the_same_nominal_geometry_without_renderer_identity() {
    let point = point2_type();
    let robotics_pose = robotics_pose2_type();
    let StructuredInfoTypeShape::Record {
        fields: robotics_fields,
        ..
    } = robotics_pose.shape()
    else {
        panic!("robotics pose must be a record")
    };
    assert_eq!(
        robotics_fields
            .iter()
            .find(|field| field.name() == "position")
            .unwrap()
            .value_type(),
        &point
    );

    let image_region = image_region2_type();
    let StructuredInfoTypeShape::Record { fields, .. } = image_region.shape() else {
        panic!("image region must be a record")
    };
    assert_eq!(
        fields
            .iter()
            .find(|field| field.name() == "region")
            .unwrap()
            .value_type(),
        &rect2_type()
    );
    let rendered = format!("{image_region:?}").to_ascii_lowercase();
    for forbidden in ["patchbay", "presenter", "framebuffer", "dom", "css"] {
        assert!(
            !rendered.contains(forbidden),
            "semantic geometry leaked {forbidden}"
        );
    }
}

fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_geometry_catalogs(&mut startup, &mut profile).unwrap();
    (startup, profile)
}

fn host() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/geometry-proof"),
        boot_id: BootId::from("boot/geometry-proof"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("std/geometry-proof@1"),
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: geometry_std_offers(),
    }
}

fn record_value<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("expected record")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value()
}

fn point_frame(value: &StructuredInfoValue) -> &str {
    let StructuredInfoValueShape::Leaf(bytes) = record_value(value, "frame").shape() else {
        panic!("frame must be text")
    };
    core::str::from_utf8(bytes).unwrap()
}

fn point_quantities(value: &StructuredInfoValue) -> (i64, i64) {
    let decode = |name| {
        let StructuredInfoValueShape::Leaf(bytes) = record_value(value, name).shape() else {
            panic!("coordinate must be a quantity")
        };
        {
            let quantity = Quantity::decode(bytes).unwrap();
            quantity.to_i64(quantity.unit()).unwrap()
        }
    };
    (decode("x"), decode("y"))
}
