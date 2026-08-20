//! Deterministic structured robotics vectors and explicit refusal boundaries.

use alloc::{string::String, string::ToString, vec, vec::Vec};
use conduit_core::{
    Quantity, QuantityDimension, QuantityUnit, StructuredFieldValue, StructuredInfoRefusal,
    StructuredInfoType, StructuredInfoValue,
};

use crate::{
    point2_value, robotics_contact_event_type, robotics_contact_phase_type,
    robotics_motion_request_type, robotics_pose2_type, robotics_pose_sample_type,
    robotics_power_telemetry_type, robotics_range_observation_type, robotics_range_sample_type,
    robotics_sample_context_type,
    robotics_twist_interval_type, vector2_type, GeometryRefusal, MAXIMUM_ROBOTICS_IDENTITY_BYTES,
};

pub const ROBOTICS_BODY_FRAME: &str = "body";

pub struct RoboticsStructuredFixture {
    pub contact: StructuredInfoValue,
    pub motion_request: StructuredInfoValue,
    pub pose: StructuredInfoValue,
    pub power: StructuredInfoValue,
    pub range: StructuredInfoValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoboticsStructuredRefusal {
    EmptyIdentity,
    IdentityTooLong,
    IncompatibleDimension { field: &'static str },
    InexactPrecision { field: &'static str },
    NegativeValue { field: &'static str },
    NegativeUncertainty { field: &'static str },
    NonPositiveInterval,
    OutsideRange { field: &'static str },
    UnsupportedFrame { expected: String, actual: String },
    Geometry(GeometryRefusal),
    Structured(StructuredInfoRefusal),
}

impl From<GeometryRefusal> for RoboticsStructuredRefusal {
    fn from(value: GeometryRefusal) -> Self {
        Self::Geometry(value)
    }
}

impl From<StructuredInfoRefusal> for RoboticsStructuredRefusal {
    fn from(value: StructuredInfoRefusal) -> Self {
        Self::Structured(value)
    }
}

pub fn deterministic_robotics_structured_fixture(
) -> Result<RoboticsStructuredFixture, RoboticsStructuredRefusal> {
    let pose = pose_sample_value(
        "sim/robot-base",
        42,
        Quantity::new(420, QuantityUnit::Millisecond),
        "map",
        Quantity::new(1_250, QuantityUnit::Millimeter),
        Quantity::new(-500, QuantityUnit::Millimeter),
        Quantity::new(90, QuantityUnit::Degree),
        Quantity::new(10, QuantityUnit::Millimeter),
        Quantity::new(1, QuantityUnit::Degree),
    )?;
    let range = range_sample_value(
        "sim/front-range",
        43,
        Quantity::new(430, QuantityUnit::Millisecond),
        "sensor/front",
        Quantity::new(420, QuantityUnit::Millimeter),
        Quantity::new(5, QuantityUnit::Millimeter),
    )?;
    let contact = contact_event_value(
        "sim/create-bumper",
        44,
        Quantity::new(440, QuantityUnit::Millisecond),
        "contact/bumper-left",
        "began",
    )?;
    let power = power_telemetry_value(
        "sim/create-power",
        45,
        Quantity::new(450, QuantityUnit::Millisecond),
        Quantity::new(750, QuantityUnit::Permille),
        Quantity::new(14_500, QuantityUnit::Millivolt),
        Quantity::new(50, QuantityUnit::Millivolt),
    )?;
    let twist = twist_interval_value(
        ROBOTICS_BODY_FRAME,
        Quantity::new(100, QuantityUnit::Millisecond),
        Quantity::new(20, QuantityUnit::Millimeter),
        Quantity::new(0, QuantityUnit::Millimeter),
        Quantity::new(2, QuantityUnit::Degree),
    )?;
    let motion_request = motion_request_value(
        "request/sim-forward",
        Quantity::new(250, QuantityUnit::Millisecond),
        twist,
    )?;
    Ok(RoboticsStructuredFixture {
        contact,
        motion_request,
        pose,
        power,
        range,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn pose_sample_value(
    source_identity: &str,
    sample_sequence: u64,
    sample_time_since_boot: Quantity,
    frame: &str,
    x: Quantity,
    y: Quantity,
    heading: Quantity,
    position_uncertainty: Quantity,
    heading_uncertainty: Quantity,
) -> Result<StructuredInfoValue, RoboticsStructuredRefusal> {
    require_identity(frame)?;
    require_exact(heading, QuantityDimension::Angle, QuantityUnit::Microdegree, "heading")?;
    require_uncertainty(position_uncertainty, QuantityDimension::Length, QuantityUnit::Micrometer, "position_uncertainty")?;
    require_uncertainty(heading_uncertainty, QuantityDimension::Angle, QuantityUnit::Microdegree, "heading_uncertainty")?;
    let position = point2_value(frame, x, y)?;
    let pose = record_value(
        robotics_pose2_type(),
        vec![("heading", quantity_value(heading)?), ("position", position)],
    )?;
    record_value(
        robotics_pose_sample_type(),
        vec![
            ("heading_uncertainty", quantity_value(heading_uncertainty)?),
            ("pose", pose),
            ("position_uncertainty", quantity_value(position_uncertainty)?),
            ("sample", sample_context_value(source_identity, sample_sequence, sample_time_since_boot)?),
        ],
    )
}

pub fn range_sample_value(
    source_identity: &str,
    sample_sequence: u64,
    sample_time_since_boot: Quantity,
    frame: &str,
    distance: Quantity,
    uncertainty: Quantity,
) -> Result<StructuredInfoValue, RoboticsStructuredRefusal> {
    require_identity(frame)?;
    let distance_mm = require_exact(
        distance,
        QuantityDimension::Length,
        QuantityUnit::Millimeter,
        "distance",
    )?;
    if !(0..=1_000_000).contains(&distance_mm) {
        return Err(RoboticsStructuredRefusal::OutsideRange { field: "distance" });
    }
    require_uncertainty(
        uncertainty,
        QuantityDimension::Length,
        QuantityUnit::Millimeter,
        "uncertainty",
    )?;
    let measurement = record_value(
        robotics_range_sample_type(),
        vec![
            ("distance", quantity_value(distance)?),
            ("frame", text_value(frame)?),
            ("uncertainty", quantity_value(uncertainty)?),
        ],
    )?;
    record_value(
        robotics_range_observation_type(),
        vec![
            ("measurement", measurement),
            (
                "sample",
                sample_context_value(source_identity, sample_sequence, sample_time_since_boot)?,
            ),
        ],
    )
}

pub fn contact_event_value(
    source_identity: &str,
    sample_sequence: u64,
    sample_time_since_boot: Quantity,
    contact_identity: &str,
    phase: &str,
) -> Result<StructuredInfoValue, RoboticsStructuredRefusal> {
    record_value(
        robotics_contact_event_type(),
        vec![
            ("contact_identity", text_value(contact_identity)?),
            ("phase", unit_variant(robotics_contact_phase_type(), phase)?),
            ("sample", sample_context_value(source_identity, sample_sequence, sample_time_since_boot)?),
        ],
    )
}

pub fn power_telemetry_value(
    source_identity: &str,
    sample_sequence: u64,
    sample_time_since_boot: Quantity,
    charge: Quantity,
    voltage: Quantity,
    voltage_uncertainty: Quantity,
) -> Result<StructuredInfoValue, RoboticsStructuredRefusal> {
    require_nonnegative(charge, QuantityDimension::Ratio, QuantityUnit::Millionth, "charge")?;
    if charge.convert(QuantityUnit::Millionth).map_err(|_| RoboticsStructuredRefusal::InexactPrecision { field: "charge" })?.value() > 1_000_000 {
        return Err(RoboticsStructuredRefusal::IncompatibleDimension { field: "charge" });
    }
    require_nonnegative(voltage, QuantityDimension::Voltage, QuantityUnit::Microvolt, "voltage")?;
    require_uncertainty(voltage_uncertainty, QuantityDimension::Voltage, QuantityUnit::Microvolt, "voltage_uncertainty")?;
    record_value(
        robotics_power_telemetry_type(),
        vec![
            ("charge", quantity_value(charge)?),
            ("sample", sample_context_value(source_identity, sample_sequence, sample_time_since_boot)?),
            ("voltage", quantity_value(voltage)?),
            ("voltage_uncertainty", quantity_value(voltage_uncertainty)?),
        ],
    )
}

pub fn twist_interval_value(
    frame: &str,
    interval: Quantity,
    linear_x: Quantity,
    linear_y: Quantity,
    angular_delta: Quantity,
) -> Result<StructuredInfoValue, RoboticsStructuredRefusal> {
    if frame != ROBOTICS_BODY_FRAME {
        return Err(RoboticsStructuredRefusal::UnsupportedFrame {
            expected: ROBOTICS_BODY_FRAME.to_string(),
            actual: frame.to_string(),
        });
    }
    let converted = require_exact(
        interval,
        QuantityDimension::Time,
        QuantityUnit::Millisecond,
        "interval",
    )?;
    if converted <= 0 {
        return Err(RoboticsStructuredRefusal::NonPositiveInterval);
    }
    require_exact(angular_delta, QuantityDimension::Angle, QuantityUnit::Microdegree, "angular_delta")?;
    let linear_delta = coordinate_value(vector2_type(), frame, linear_x, linear_y)?;
    record_value(
        robotics_twist_interval_type(),
        vec![
            ("angular_delta", quantity_value(angular_delta)?),
            ("frame", text_value(frame)?),
            ("interval", quantity_value(interval)?),
            ("linear_delta", linear_delta),
        ],
    )
}

pub fn motion_request_value(
    request_identity: &str,
    expires_after: Quantity,
    twist: StructuredInfoValue,
) -> Result<StructuredInfoValue, RoboticsStructuredRefusal> {
    require_identity(request_identity)?;
    let converted = require_exact(
        expires_after,
        QuantityDimension::Time,
        QuantityUnit::Millisecond,
        "expires_after",
    )?;
    if converted <= 0 {
        return Err(RoboticsStructuredRefusal::NonPositiveInterval);
    }
    record_value(
        robotics_motion_request_type(),
        vec![
            ("expires_after", quantity_value(expires_after)?),
            ("request_identity", text_value(request_identity)?),
            ("twist", twist),
        ],
    )
}

fn sample_context_value(
    source_identity: &str,
    sample_sequence: u64,
    sample_time_since_boot: Quantity,
) -> Result<StructuredInfoValue, RoboticsStructuredRefusal> {
    require_identity(source_identity)?;
    require_nonnegative(
        sample_time_since_boot,
        QuantityDimension::Time,
        QuantityUnit::Millisecond,
        "sample_time_since_boot",
    )?;
    record_value(
        robotics_sample_context_type(),
        vec![
            ("sample_sequence", count_value(sample_sequence)?),
            ("sample_time_since_boot", quantity_value(sample_time_since_boot)?),
            ("source_identity", text_value(source_identity)?),
        ],
    )
}

fn coordinate_value(
    value_type: StructuredInfoType,
    frame: &str,
    x: Quantity,
    y: Quantity,
) -> Result<StructuredInfoValue, RoboticsStructuredRefusal> {
    require_exact(x, QuantityDimension::Length, QuantityUnit::Micrometer, "linear_x")?;
    require_exact(y, QuantityDimension::Length, QuantityUnit::Micrometer, "linear_y")?;
    record_value(
        value_type,
        vec![
            ("frame", text_value(frame)?),
            ("x", quantity_value(x)?),
            ("y", quantity_value(y)?),
        ],
    )
}

fn require_uncertainty(
    value: Quantity,
    dimension: QuantityDimension,
    canonical: QuantityUnit,
    field: &'static str,
) -> Result<(), RoboticsStructuredRefusal> {
    if require_exact(value, dimension, canonical, field)? < 0 {
        return Err(RoboticsStructuredRefusal::NegativeUncertainty { field });
    }
    Ok(())
}

fn require_nonnegative(
    value: Quantity,
    dimension: QuantityDimension,
    canonical: QuantityUnit,
    field: &'static str,
) -> Result<(), RoboticsStructuredRefusal> {
    if require_exact(value, dimension, canonical, field)? < 0 {
        return Err(RoboticsStructuredRefusal::NegativeValue { field });
    }
    Ok(())
}

fn require_exact(
    value: Quantity,
    dimension: QuantityDimension,
    canonical: QuantityUnit,
    field: &'static str,
) -> Result<i64, RoboticsStructuredRefusal> {
    if value.dimension() != dimension {
        return Err(RoboticsStructuredRefusal::IncompatibleDimension { field });
    }
    value
        .convert(canonical)
        .map(|value| value.value())
        .map_err(|_| RoboticsStructuredRefusal::InexactPrecision { field })
}

fn require_identity(value: &str) -> Result<(), RoboticsStructuredRefusal> {
    if value.is_empty() {
        return Err(RoboticsStructuredRefusal::EmptyIdentity);
    }
    if value.len() > MAXIMUM_ROBOTICS_IDENTITY_BYTES {
        return Err(RoboticsStructuredRefusal::IdentityTooLong);
    }
    Ok(())
}

fn text_value(value: &str) -> Result<StructuredInfoValue, RoboticsStructuredRefusal> {
    require_identity(value)?;
    leaf_value("value/text@1", value.as_bytes().to_vec())
}

fn count_value(value: u64) -> Result<StructuredInfoValue, RoboticsStructuredRefusal> {
    leaf_value("value/count@1", value.to_string().into_bytes())
}

fn quantity_value(value: Quantity) -> Result<StructuredInfoValue, RoboticsStructuredRefusal> {
    leaf_value(conduit_core::QUANTITY_INFO_ID, value.encode().to_vec())
}

fn unit_variant(
    value_type: StructuredInfoType,
    tag: &str,
) -> Result<StructuredInfoValue, RoboticsStructuredRefusal> {
    Ok(StructuredInfoValue::variant(
        value_type,
        tag,
        leaf_value("value/unit@1", Vec::new())?,
    )?)
}

fn leaf_value(
    kind: &str,
    bytes: Vec<u8>,
) -> Result<StructuredInfoValue, RoboticsStructuredRefusal> {
    Ok(StructuredInfoValue::leaf(
        StructuredInfoType::leaf(conduit_core::kind_id(kind))?,
        bytes,
    )?)
}

fn record_value(
    value_type: StructuredInfoType,
    fields: Vec<(&str, StructuredInfoValue)>,
) -> Result<StructuredInfoValue, RoboticsStructuredRefusal> {
    Ok(StructuredInfoValue::record(
        value_type,
        fields
            .into_iter()
            .map(|(name, value)| StructuredFieldValue::new(name, value))
            .collect::<Result<Vec<_>, _>>()?,
    )?)
}
