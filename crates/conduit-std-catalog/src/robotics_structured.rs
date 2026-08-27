//! Structured portable robotics observations and actuator intent.
//!
//! Create OI, GPIO, I2C, SPI, UART, report layouts, and provider-specific
//! precision remain below these finite semantic values.

use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, StructuredFieldType, StructuredInfoType, StructuredVariantCase, QUANTITY_INFO_ID,
};

use conduit_presentation::{robotics_pose2_type, vector2_type};

pub const ROBOTICS_SAMPLE_CONTEXT_TYPE: &str = "RoboticsSampleContext";
pub const ROBOTICS_POSE_SAMPLE_TYPE: &str = "RoboticsPoseSample";
pub const ROBOTICS_TWIST_INTERVAL_TYPE: &str = "RoboticsTwistInterval";
pub const ROBOTICS_RANGE_OBSERVATION_TYPE: &str = "RoboticsRangeObservation";
pub const ROBOTICS_CONTACT_EVENT_TYPE: &str = "RoboticsContactEvent";
pub const ROBOTICS_POWER_TELEMETRY_TYPE: &str = "RoboticsPowerTelemetry";
pub const ROBOTICS_MOTION_REQUEST_TYPE: &str = "RoboticsMotionRequest";
pub const MAXIMUM_ROBOTICS_IDENTITY_BYTES: usize = 64;

fn leaf(kind: &str) -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(kind)).expect("reviewed robotics leaf")
}

fn text_type() -> StructuredInfoType {
    leaf("value/text@1")
}

fn count_type() -> StructuredInfoType {
    leaf("value/count@1")
}

fn quantity_type() -> StructuredInfoType {
    leaf(QUANTITY_INFO_ID)
}

fn unit_type() -> StructuredInfoType {
    leaf("value/unit@1")
}

fn field(name: &str, value_type: StructuredInfoType) -> StructuredFieldType {
    StructuredFieldType::new(name, value_type).expect("reviewed robotics field")
}

fn case(name: &str) -> StructuredVariantCase {
    StructuredVariantCase::new(name, unit_type()).expect("reviewed robotics case")
}

fn record(kind: &str, fields: Vec<StructuredFieldType>) -> StructuredInfoType {
    StructuredInfoType::record(kind_id(kind), fields).expect("reviewed robotics record")
}

pub fn robotics_sample_context_type() -> StructuredInfoType {
    record(
        "robotics/sample-context@1",
        vec![
            field("sample_sequence", count_type()),
            field("sample_time_since_boot", quantity_type()),
            field("source_identity", text_type()),
        ],
    )
}

pub fn robotics_pose_sample_type() -> StructuredInfoType {
    record(
        "robotics/pose-sample@1",
        vec![
            field("heading_uncertainty", quantity_type()),
            field("pose", robotics_pose2_type()),
            field("position_uncertainty", quantity_type()),
            field("sample", robotics_sample_context_type()),
        ],
    )
}

/// Velocity without inventing compound Quantity units: exact displacement and
/// angular change over one explicit positive interval.
pub fn robotics_twist_interval_type() -> StructuredInfoType {
    record(
        "robotics/twist-interval@1",
        vec![
            field("angular_delta", quantity_type()),
            field("frame", text_type()),
            field("interval", quantity_type()),
            field("linear_delta", vector2_type()),
        ],
    )
}

pub fn robotics_range_observation_type() -> StructuredInfoType {
    record(
        "robotics/range-observation@1",
        vec![
            field("measurement", crate::robotics_range_sample_type()),
            field("sample", robotics_sample_context_type()),
        ],
    )
}

pub fn robotics_contact_phase_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("robotics/contact-phase@1"),
        vec![case("began"), case("ended")],
    )
    .expect("reviewed contact phase")
}

pub fn robotics_contact_event_type() -> StructuredInfoType {
    record(
        "robotics/contact-event@1",
        vec![
            field("contact_identity", text_type()),
            field("phase", robotics_contact_phase_type()),
            field("sample", robotics_sample_context_type()),
        ],
    )
}

pub fn robotics_power_telemetry_type() -> StructuredInfoType {
    record(
        "robotics/power-telemetry@1",
        vec![
            field("charge", quantity_type()),
            field("sample", robotics_sample_context_type()),
            field("voltage", quantity_type()),
            field("voltage_uncertainty", quantity_type()),
        ],
    )
}

pub fn robotics_motion_request_type() -> StructuredInfoType {
    record(
        "robotics/motion-request@1",
        vec![
            field("expires_after", quantity_type()),
            field("request_identity", text_type()),
            field("twist", robotics_twist_interval_type()),
        ],
    )
}

pub fn robotics_structured_registered_types() -> Vec<(&'static str, StructuredInfoType)> {
    vec![
        (ROBOTICS_SAMPLE_CONTEXT_TYPE, robotics_sample_context_type()),
        (ROBOTICS_POSE_SAMPLE_TYPE, robotics_pose_sample_type()),
        (ROBOTICS_TWIST_INTERVAL_TYPE, robotics_twist_interval_type()),
        (
            ROBOTICS_RANGE_OBSERVATION_TYPE,
            robotics_range_observation_type(),
        ),
        (ROBOTICS_CONTACT_EVENT_TYPE, robotics_contact_event_type()),
        (
            ROBOTICS_POWER_TELEMETRY_TYPE,
            robotics_power_telemetry_type(),
        ),
        (ROBOTICS_MOTION_REQUEST_TYPE, robotics_motion_request_type()),
    ]
}
