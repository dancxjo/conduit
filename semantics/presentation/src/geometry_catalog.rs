//! Form catalog for portable finite geometry semantics.

use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, ConfigurationValue, KindContractRevision, PortDescriptor, PortDirection,
    PortTemporal, Quantity, QuantityUnit, StructuredConfigurationValue, StructuredInfoType,
    StructuredInfoValue,
};
use conduit_form::{
    ConfigurationField, ConfigurationRule, KindDefinition, KindSignature, StartupParameterSignature,
};

use crate::{
    path2_type, point2_type, point2_value, transform2_value, EXTENT2_TYPE, IMAGE_REGION2_TYPE,
    PATH2_FOUR_TYPE, POINT2_TYPE, POINT3_TYPE, RECT2_TYPE, ROBOTICS_POSE2_TYPE, TRANSFORM2_TYPE,
    VECTOR2_TYPE, VECTOR3_TYPE,
};

pub const POINT2_LITERAL_KIND: &str = "geometry/point2";
pub const APPLY_TRANSFORM2_KIND: &str = "geometry/apply-transform2";
pub const TRANSFORM_PATH2_FOUR_KIND: &str = "geometry/transform-path2-four";
pub const CAPTURE_BOUNDED_STROKE_KIND: &str = "geometry/capture-bounded-stroke";
pub const GEOMETRY_REVISION: &str = "conduit.std/geometry-spatial@1";

pub fn install_geometry_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    for (name, value_type) in geometry_types() {
        startup
            .insert_structured_type(name, value_type)
            .map_err(|error| error.to_string())?;
    }
    let point = point2_type();
    let path = path2_type(4).expect("four-point path is bounded");
    insert_kind(
        startup,
        profile,
        POINT2_LITERAL_KIND,
        vec![],
        vec![geometry_port("point", &point, PortDirection::Output)],
        Some(("value", POINT2_TYPE, default_point2()?)),
    )?;
    insert_kind(
        startup,
        profile,
        APPLY_TRANSFORM2_KIND,
        vec![geometry_port("point", &point, PortDirection::Input)],
        vec![geometry_port("point", &point, PortDirection::Output)],
        Some(("transform", TRANSFORM2_TYPE, default_transform2()?)),
    )?;
    insert_kind(
        startup,
        profile,
        TRANSFORM_PATH2_FOUR_KIND,
        vec![geometry_port("path", &path, PortDirection::Input)],
        vec![geometry_port("path", &path, PortDirection::Output)],
        Some(("transform", TRANSFORM2_TYPE, default_transform2()?)),
    )?;
    let stroke = path2_type(4).expect("four-point stroke bound is reviewed");
    startup.insert(KindSignature {
        kind: CAPTURE_BOUNDED_STROKE_KIND.into(),
        startup_parameters: vec![],
    })?;
    profile
        .insert(KindDefinition {
            kind_id: kind_id(CAPTURE_BOUNDED_STROKE_KIND),
            kind_contract_revision: KindContractRevision::from(GEOMETRY_REVISION),
            inputs: vec![flow_geometry_port("point", &point, PortDirection::Input)],
            outputs: vec![geometry_port("stroke", &stroke, PortDirection::Output)],
            configuration: vec![],
        })
        .map_err(|error| error.to_string())
}

pub fn geometry_types() -> Vec<(&'static str, StructuredInfoType)> {
    vec![
        (POINT2_TYPE, crate::point2_type()),
        (POINT3_TYPE, crate::point3_type()),
        (VECTOR2_TYPE, crate::vector2_type()),
        (VECTOR3_TYPE, crate::vector3_type()),
        (EXTENT2_TYPE, crate::extent2_type()),
        (RECT2_TYPE, crate::rect2_type()),
        (TRANSFORM2_TYPE, crate::transform2_type()),
        (PATH2_FOUR_TYPE, crate::path2_type(4).unwrap()),
        (ROBOTICS_POSE2_TYPE, crate::robotics_pose2_type()),
        (IMAGE_REGION2_TYPE, crate::image_region2_type()),
    ]
}

fn insert_kind(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
    kind: &str,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
    configuration: Option<(&str, &str, StructuredInfoValue)>,
) -> Result<(), String> {
    startup
        .insert(KindSignature {
            kind: kind.into(),
            startup_parameters: configuration
                .as_ref()
                .map(|(name, type_name, _)| StartupParameterSignature {
                    name: (*name).into(),
                    value_type: (*type_name).into(),
                    default: None,
                })
                .into_iter()
                .collect(),
        })
        .map_err(|error| error.to_string())?;
    let configuration = configuration
        .map(|(name, _, value)| {
            let value_type = value.value_type().clone();
            let profile = value_type.profile().map_err(|error| format!("{error:?}"))?;
            let canonical = value
                .canonical_bytes()
                .map_err(|error| format!("{error:?}"))?;
            Ok::<ConfigurationField, String>(ConfigurationField {
                key: name.into(),
                default_value: ConfigurationValue::Structured(
                    StructuredConfigurationValue::new(profile.value_kind().clone(), canonical)
                        .ok_or_else(|| "geometry default exceeds structured bound".to_string())?,
                ),
                validation: ConfigurationRule::Structured {
                    profile: profile.value_kind().clone(),
                },
            })
        })
        .transpose()?
        .into_iter()
        .collect();
    profile
        .insert(KindDefinition {
            kind_id: kind_id(kind),
            kind_contract_revision: KindContractRevision::from(GEOMETRY_REVISION),
            inputs,
            outputs,
            configuration,
        })
        .map_err(|error| error.to_string())
}

pub fn geometry_port(
    name: &str,
    value_type: &StructuredInfoType,
    direction: PortDirection,
) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: value_type
            .profile()
            .expect("bounded geometry type")
            .value_kind()
            .clone(),
        direction,
        temporal: PortTemporal::Value,
    }
}

fn flow_geometry_port(
    name: &str,
    value_type: &StructuredInfoType,
    direction: PortDirection,
) -> PortDescriptor {
    let mut port = geometry_port(name, value_type, direction);
    port.temporal = PortTemporal::Flow { closes: true };
    port
}

fn default_point2() -> Result<StructuredInfoValue, String> {
    point2_value(
        "geometry/example",
        Quantity::new(0, QuantityUnit::Millimeter),
        Quantity::new(0, QuantityUnit::Millimeter),
    )
    .map_err(|error| format!("{error:?}"))
}

fn default_transform2() -> Result<StructuredInfoValue, String> {
    transform2_value(
        "geometry/example",
        "geometry/example",
        Quantity::new(0, QuantityUnit::Millimeter),
        Quantity::new(0, QuantityUnit::Millimeter),
    )
    .map_err(|error| format!("{error:?}"))
}
