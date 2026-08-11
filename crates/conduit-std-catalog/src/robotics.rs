use super::{
    StandardConfigurationField, StandardConfigurationRule, StandardKindContract, TerminalBehavior,
};
use alloc::string::ToString;
use alloc::{format, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ConfigurationValue, ExecutionProfileId, ImplementationId, KindContractRevision, PortDescriptor,
    PortDirection, PortTemporal, BOOL_INFO_ID, MAXIMUM_BATTERY_MILLIVOLTS,
    MAXIMUM_OBSERVATION_AGE_MS, MAXIMUM_ODOMETRY_MM, MAXIMUM_RANGE_MM, PI_MICRORADIANS,
    ROBOTICS_BATTERY_INFO_ID, ROBOTICS_ODOMETRY_INFO_ID, ROBOTICS_ORIENTATION_INFO_ID,
    ROBOTICS_RANGE_INFO_ID, SCALAR_INFO_ID,
};

pub const ROBOTICS_OBSERVE_BUMP_KIND: &str = "robotics/observe-bump";
pub const ROBOTICS_OBSERVE_IMU_KIND: &str = "robotics/observe-imu";
pub const ROBOTICS_OBSERVE_RANGE_KIND: &str = "robotics/observe-range";
pub const ROBOTICS_OBSERVE_ODOMETRY_KIND: &str = "robotics/observe-odometry";
pub const ROBOTICS_OBSERVE_BATTERY_KIND: &str = "robotics/observe-battery";
pub const ROBOTICS_VELOCITY_INTENT_KIND: &str = "robotics/velocity-intent";
pub const ROBOTICS_DRIVE_DIFFERENTIAL_KIND: &str = "robotics/drive-differential";

pub const ROBOTICS_OBSERVE_BUMP_REVISION: &str = "conduit.std/robotics-observe-bump@1";
pub const ROBOTICS_OBSERVE_IMU_REVISION: &str = "conduit.std/robotics-observe-imu@1";
pub const ROBOTICS_OBSERVE_RANGE_REVISION: &str = "conduit.std/robotics-observe-range@1";
pub const ROBOTICS_OBSERVE_ODOMETRY_REVISION: &str = "conduit.std/robotics-observe-odometry@1";
pub const ROBOTICS_OBSERVE_BATTERY_REVISION: &str = "conduit.std/robotics-observe-battery@1";
pub const ROBOTICS_VELOCITY_INTENT_REVISION: &str = "conduit.std/robotics-velocity-intent@1";
pub const ROBOTICS_DRIVE_DIFFERENTIAL_REVISION: &str = "conduit.std/robotics-drive-differential@1";

pub const ROBOTICS_AVAILABILITY_KEY: &str = "availability";
pub const ROBOTICS_AVAILABILITY_FRESH: &str = "fresh";
pub const ROBOTICS_AVAILABILITY_MISSING: &str = "missing";
pub const ROBOTICS_AVAILABILITY_STALE: &str = "stale";
pub const ROBOTICS_MAXIMUM_VELOCITY_MICROUNITS: i64 = 5_000_000;

pub const ROBOTICS_EXECUTION_PROFILE: &str = "conduit.std/robotics-prewake-sim-kernel@1";
pub const ROBOTICS_ARTIFACT: &str = "conduit-std-host/robotics-prewake-sim@1";
pub const ROBOTICS_OBSERVE_BUMP_IMPLEMENTATION: &str = "std/kernel-robotics-prewake-observe-bump@1";
pub const ROBOTICS_OBSERVE_IMU_IMPLEMENTATION: &str = "std/kernel-robotics-prewake-observe-imu@1";
pub const ROBOTICS_OBSERVE_RANGE_IMPLEMENTATION: &str =
    "std/kernel-robotics-prewake-observe-range@1";
pub const ROBOTICS_OBSERVE_ODOMETRY_IMPLEMENTATION: &str =
    "std/kernel-robotics-prewake-observe-odometry@1";
pub const ROBOTICS_OBSERVE_BATTERY_IMPLEMENTATION: &str =
    "std/kernel-robotics-prewake-observe-battery@1";
pub const ROBOTICS_VELOCITY_INTENT_IMPLEMENTATION: &str =
    "std/kernel-robotics-prewake-velocity-intent@1";
pub const ROBOTICS_DRIVE_DIFFERENTIAL_IMPLEMENTATION: &str =
    "std/kernel-robotics-prewake-drive-differential@1";
const MAXIMUM_VALUE_BYTES: u32 = 12;

pub fn robotics_observe_bump_contract() -> StandardKindContract {
    source_contract(
        ROBOTICS_OBSERVE_BUMP_KIND,
        "Simulated bump observation",
        "Emit one exact current bumper state from a bounded PREWAKE simulation.",
        vec![current_output("observation", BOOL_INFO_ID)],
        vec![
            availability_field(),
            text_field("state", "clear", &["clear", "pressed"]),
        ],
        "bump: robotics/observe-bump(state = \"pressed\")",
    )
}

pub fn robotics_observe_imu_contract() -> StandardKindContract {
    source_contract(
        ROBOTICS_OBSERVE_IMU_KIND,
        "Simulated body orientation",
        "Emit one bounded roll/pitch/yaw observation in body-frame microradians.",
        vec![current_output("orientation", ROBOTICS_ORIENTATION_INFO_ID)],
        vec![
            availability_field(),
            i64_field(
                "roll-microradians",
                0,
                -i64::from(PI_MICRORADIANS),
                i64::from(PI_MICRORADIANS),
            ),
            i64_field(
                "pitch-microradians",
                0,
                -i64::from(conduit_core::HALF_PI_MICRORADIANS),
                i64::from(conduit_core::HALF_PI_MICRORADIANS),
            ),
            i64_field(
                "yaw-microradians",
                0,
                -i64::from(PI_MICRORADIANS),
                i64::from(PI_MICRORADIANS),
            ),
        ],
        "imu: robotics/observe-imu",
    )
}

pub fn robotics_observe_range_contract() -> StandardKindContract {
    source_contract(
        ROBOTICS_OBSERVE_RANGE_KIND,
        "Simulated forward range",
        "Emit one sensor-forward range observation with millimeter distance and bounded age.",
        vec![current_output("range", ROBOTICS_RANGE_INFO_ID)],
        vec![
            availability_field(),
            u64_field("distance-mm", 1_000, 0, u64::from(MAXIMUM_RANGE_MM)),
            u64_field("age-ms", 0, 0, u64::from(MAXIMUM_OBSERVATION_AGE_MS)),
        ],
        "range: robotics/observe-range(distance-mm = 500)",
    )
}

pub fn robotics_observe_odometry_contract() -> StandardKindContract {
    source_contract(
        ROBOTICS_OBSERVE_ODOMETRY_KIND,
        "Simulated local odometry",
        "Emit one start-local forward/lateral/yaw odometry observation.",
        vec![current_output("odometry", ROBOTICS_ODOMETRY_INFO_ID)],
        vec![
            availability_field(),
            i64_field(
                "forward-mm",
                0,
                -i64::from(MAXIMUM_ODOMETRY_MM),
                i64::from(MAXIMUM_ODOMETRY_MM),
            ),
            i64_field(
                "lateral-mm",
                0,
                -i64::from(MAXIMUM_ODOMETRY_MM),
                i64::from(MAXIMUM_ODOMETRY_MM),
            ),
            i64_field(
                "yaw-microradians",
                0,
                -i64::from(PI_MICRORADIANS),
                i64::from(PI_MICRORADIANS),
            ),
        ],
        "odometry: robotics/observe-odometry",
    )
}

pub fn robotics_observe_battery_contract() -> StandardKindContract {
    source_contract(
        ROBOTICS_OBSERVE_BATTERY_KIND,
        "Simulated battery observation",
        "Emit one charge-permille and millivolt battery observation.",
        vec![current_output("battery", ROBOTICS_BATTERY_INFO_ID)],
        vec![
            availability_field(),
            u64_field("charge-permille", 1_000, 0, 1_000),
            u64_field(
                "millivolts",
                12_000,
                0,
                u64::from(MAXIMUM_BATTERY_MILLIVOLTS),
            ),
        ],
        "battery: robotics/observe-battery",
    )
}

pub fn robotics_velocity_intent_contract() -> StandardKindContract {
    source_contract(
        ROBOTICS_VELOCITY_INTENT_KIND,
        "Simulated body velocity intent",
        "Emit bounded body-forward linear and counter-clockwise angular scalar intent.",
        vec![
            current_output("linear", SCALAR_INFO_ID),
            current_output("angular", SCALAR_INFO_ID),
        ],
        vec![
            i64_field(
                "linear-microunits",
                0,
                -ROBOTICS_MAXIMUM_VELOCITY_MICROUNITS,
                ROBOTICS_MAXIMUM_VELOCITY_MICROUNITS,
            ),
            i64_field(
                "angular-microunits",
                0,
                -ROBOTICS_MAXIMUM_VELOCITY_MICROUNITS,
                ROBOTICS_MAXIMUM_VELOCITY_MICROUNITS,
            ),
        ],
        "intent: robotics/velocity-intent(linear-microunits = 500000)",
    )
}

pub fn robotics_drive_differential_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(ROBOTICS_DRIVE_DIFFERENTIAL_KIND),
        plain_name: "Simulated differential drive".to_string(),
        summary: "Consume bounded linear/angular intent as a PREWAKE projection only; no physical effect or authority is implied."
            .to_string(),
        inputs: vec![
            current_input("linear", SCALAR_INFO_ID),
            current_input("angular", SCALAR_INFO_ID),
            current_input("bumper-pressed", BOOL_INFO_ID),
            current_input("forward-range", ROBOTICS_RANGE_INFO_ID),
        ],
        outputs: Vec::new(),
        configuration: vec![
            u64_field("minimum-clearance-mm", 250, 0, u64::from(MAXIMUM_RANGE_MM)),
            u64_field(
                "maximum-range-age-ms",
                1_000,
                0,
                u64::from(MAXIMUM_OBSERVATION_AGE_MS),
            ),
        ],
        limits: limits(),
        terminal_behavior: TerminalBehavior::SimulatedDriveProjectionCompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "drive: robotics/drive-differential(minimum-clearance-mm = 250)".to_string(),
    }
}

pub fn robotics_observe_bump_offer() -> CapabilityOffer {
    offer(
        robotics_observe_bump_contract(),
        ROBOTICS_OBSERVE_BUMP_REVISION,
        "observe-bump",
        ROBOTICS_OBSERVE_BUMP_IMPLEMENTATION,
    )
}

pub fn robotics_observe_imu_offer() -> CapabilityOffer {
    offer(
        robotics_observe_imu_contract(),
        ROBOTICS_OBSERVE_IMU_REVISION,
        "observe-imu",
        ROBOTICS_OBSERVE_IMU_IMPLEMENTATION,
    )
}

pub fn robotics_observe_range_offer() -> CapabilityOffer {
    offer(
        robotics_observe_range_contract(),
        ROBOTICS_OBSERVE_RANGE_REVISION,
        "observe-range",
        ROBOTICS_OBSERVE_RANGE_IMPLEMENTATION,
    )
}

pub fn robotics_observe_odometry_offer() -> CapabilityOffer {
    offer(
        robotics_observe_odometry_contract(),
        ROBOTICS_OBSERVE_ODOMETRY_REVISION,
        "observe-odometry",
        ROBOTICS_OBSERVE_ODOMETRY_IMPLEMENTATION,
    )
}

pub fn robotics_observe_battery_offer() -> CapabilityOffer {
    offer(
        robotics_observe_battery_contract(),
        ROBOTICS_OBSERVE_BATTERY_REVISION,
        "observe-battery",
        ROBOTICS_OBSERVE_BATTERY_IMPLEMENTATION,
    )
}

pub fn robotics_velocity_intent_offer() -> CapabilityOffer {
    offer(
        robotics_velocity_intent_contract(),
        ROBOTICS_VELOCITY_INTENT_REVISION,
        "velocity-intent",
        ROBOTICS_VELOCITY_INTENT_IMPLEMENTATION,
    )
}

pub fn robotics_drive_differential_offer() -> CapabilityOffer {
    offer(
        robotics_drive_differential_contract(),
        ROBOTICS_DRIVE_DIFFERENTIAL_REVISION,
        "drive-differential",
        ROBOTICS_DRIVE_DIFFERENTIAL_IMPLEMENTATION,
    )
}

#[cfg(any(feature = "form-catalog", test))]
pub(crate) fn robotics_contracts_with_revisions() -> [(StandardKindContract, &'static str); 7] {
    [
        (
            robotics_observe_bump_contract(),
            ROBOTICS_OBSERVE_BUMP_REVISION,
        ),
        (
            robotics_observe_imu_contract(),
            ROBOTICS_OBSERVE_IMU_REVISION,
        ),
        (
            robotics_observe_range_contract(),
            ROBOTICS_OBSERVE_RANGE_REVISION,
        ),
        (
            robotics_observe_odometry_contract(),
            ROBOTICS_OBSERVE_ODOMETRY_REVISION,
        ),
        (
            robotics_observe_battery_contract(),
            ROBOTICS_OBSERVE_BATTERY_REVISION,
        ),
        (
            robotics_velocity_intent_contract(),
            ROBOTICS_VELOCITY_INTENT_REVISION,
        ),
        (
            robotics_drive_differential_contract(),
            ROBOTICS_DRIVE_DIFFERENTIAL_REVISION,
        ),
    ]
}

fn source_contract(
    kind: &str,
    name: &str,
    summary: &str,
    outputs: Vec<PortDescriptor>,
    configuration: Vec<StandardConfigurationField>,
    example: &str,
) -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(kind),
        plain_name: name.to_string(),
        summary: summary.to_string(),
        inputs: Vec::new(),
        outputs,
        configuration,
        limits: limits(),
        terminal_behavior: TerminalBehavior::SimulatedCurrentObservationEmitsOnce,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: example.to_string(),
    }
}

fn offer(
    contract: StandardKindContract,
    revision: &str,
    slug: &str,
    implementation: &str,
) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: contract
            .configuration
            .iter()
            .map(|field| conduit_core::FaceStartupParameter {
                name: field.key.clone(),
                value_type: configuration_type(field).to_string(),
                has_default: true,
            })
            .collect(),
        shorthand: None,
        capability_id: CapabilityId::from(format!("robotics-prewake-sim-{slug}")),
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(revision),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(ROBOTICS_EXECUTION_PROFILE),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(ROBOTICS_ARTIFACT),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

fn current_output(name: &str, info: &str) -> PortDescriptor {
    port(name, info, PortDirection::Output)
}

fn current_input(name: &str, info: &str) -> PortDescriptor {
    port(name, info, PortDirection::Input)
}

fn port(name: &str, info: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(info),
        direction,
        temporal: PortTemporal::Current,
    }
}

fn limits() -> CapabilityLimits {
    CapabilityLimits {
        max_active_instances: 16,
        max_queue_items: 1,
        max_queue_bytes: MAXIMUM_VALUE_BYTES,
    }
}

fn availability_field() -> StandardConfigurationField {
    text_field(
        ROBOTICS_AVAILABILITY_KEY,
        ROBOTICS_AVAILABILITY_FRESH,
        &[
            ROBOTICS_AVAILABILITY_FRESH,
            ROBOTICS_AVAILABILITY_MISSING,
            ROBOTICS_AVAILABILITY_STALE,
        ],
    )
}

fn text_field(key: &str, default: &str, values: &[&str]) -> StandardConfigurationField {
    StandardConfigurationField {
        key: key.to_string(),
        default_value: ConfigurationValue::Text(default.to_string()),
        rule: StandardConfigurationRule::TextOneOf {
            values: values.iter().map(|value| (*value).to_string()).collect(),
        },
    }
}

fn u64_field(key: &str, default: u64, minimum: u64, maximum: u64) -> StandardConfigurationField {
    StandardConfigurationField {
        key: key.to_string(),
        default_value: ConfigurationValue::U64(default),
        rule: StandardConfigurationRule::U64Range { minimum, maximum },
    }
}

fn i64_field(key: &str, default: i64, minimum: i64, maximum: i64) -> StandardConfigurationField {
    StandardConfigurationField {
        key: key.to_string(),
        default_value: ConfigurationValue::I64(default),
        rule: StandardConfigurationRule::I64Range { minimum, maximum },
    }
}

pub(crate) fn configuration_type(field: &StandardConfigurationField) -> &'static str {
    match &field.default_value {
        ConfigurationValue::Text(_) => "Text",
        ConfigurationValue::U64(_) => "Count",
        ConfigurationValue::I64(_) => "Scalar",
        _ => unreachable!("robotics configuration is finite text/integer"),
    }
}

#[cfg(test)]
#[path = "robotics_tests.rs"]
mod tests;
