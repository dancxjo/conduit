//! Pete-owned authoring catalog for Pete's current self-state composition.

use conduit_core::{
    kind_id, port_id, ConfigurationValue, KindIdentity, PortDescriptor, PortDirection,
    PortTemporal, StructuredFieldType, StructuredInfoType, StructuredVariantCase,
};
use conduit_form::{
    KindConfigurationField, KindConfigurationRule, KindProjection, KindSignature, ProfileCatalog,
    StartupCatalog, StartupParameterSignature,
};
use std::{
    string::{String, ToString},
    vec,
    vec::Vec,
};

pub const HOMEOSTASIS_REDUCE_KIND: &str = "pete/reduce-homeostasis";
pub const HOMEOSTASIS_REDUCE_REVISION: &str = "conduit.pete/homeostasis-reduce@1";

fn leaf(kind: &str) -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(kind)).expect("reviewed homeostasis leaf")
}

fn field(name: &str, value_type: StructuredInfoType) -> StructuredFieldType {
    StructuredFieldType::new(name, value_type).expect("reviewed homeostasis field")
}

fn case(tag: &str, payload_type: StructuredInfoType) -> StructuredVariantCase {
    StructuredVariantCase::new(tag, payload_type).expect("reviewed homeostasis case")
}

fn record(kind: &str, fields: Vec<StructuredFieldType>) -> StructuredInfoType {
    StructuredInfoType::record(kind_id(kind), fields).expect("reviewed homeostasis record")
}

fn temporal_instant_type() -> StructuredInfoType {
    record(
        "time/instant@1",
        vec![
            field("clock_basis", leaf("value/text")),
            field("resolution_ticks", leaf("value/count")),
            field(
                "scale",
                StructuredInfoType::variant(
                    kind_id("time/scale@1"),
                    ["seconds", "milliseconds", "microseconds", "nanoseconds"]
                        .into_iter()
                        .map(|tag| case(tag, leaf("value/unit")))
                        .collect(),
                )
                .unwrap(),
            ),
            field("ticks", leaf("value/count")),
            field("uncertainty_ticks", leaf("value/count")),
        ],
    )
}

fn optional_calibration_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("experience/optional-calibration-profile@1"),
        vec![
            case("known", leaf("value/text")),
            case("none", leaf("value/unit")),
        ],
    )
    .unwrap()
}

/// The shared observation envelope is domain-neutral; only `payload_type`
/// belongs to Homeostasis.
fn observation_envelope_type(kind: &str, payload_type: StructuredInfoType) -> StructuredInfoType {
    let present = record(
        &format!("{kind}/present@1"),
        vec![
            field("observed_at", temporal_instant_type()),
            field("observation_sign_identity", leaf("value/text")),
            field("value", payload_type),
        ],
    );
    let availability = StructuredInfoType::variant(
        kind_id(&format!("{kind}/availability@1")),
        vec![
            case("missing", leaf("value/unit")),
            case("present", present),
            case("unavailable", leaf("value/unit")),
        ],
    )
    .unwrap();
    StructuredInfoType::record(
        kind_id(kind),
        vec![
            field("availability", availability),
            field("calibration_profile", optional_calibration_type()),
            field("freshness_limit_ticks", leaf("value/count")),
            field("source_identity", leaf("value/text")),
            field("uncertainty_permille", leaf("value/count")),
        ],
    )
    .expect("reviewed bounded homeostasis input")
}

pub fn homeostasis_registered_types() -> Vec<(&'static str, StructuredInfoType)> {
    vec![
        (
            "PetePowerObservation",
            observation_envelope_type(
                "pete/power-observation@1",
                record(
                    "observation/energy-reserve-and-charging@1",
                    vec![
                        field("charging", leaf("value/bool")),
                        field("reserve_permille", leaf("value/count")),
                    ],
                ),
            ),
        ),
        (
            "PeteThermalObservation",
            observation_envelope_type(
                "pete/thermal-observation@1",
                record(
                    "observation/temperature-milli-celsius@1",
                    vec![field("milli_celsius", leaf("value/scalar"))],
                ),
            ),
        ),
        (
            "PeteComputePressureObservation",
            observation_envelope_type(
                "pete/compute-pressure-observation@1",
                record(
                    "observation/pressure-permille@1",
                    vec![field("pressure_permille", leaf("value/count"))],
                ),
            ),
        ),
        (
            "PeteStoragePressureObservation",
            observation_envelope_type(
                "pete/storage-pressure-observation@1",
                record(
                    "observation/pressure-permille@1",
                    vec![field("pressure_permille", leaf("value/count"))],
                ),
            ),
        ),
        (
            "PeteMotionSafetyObservation",
            observation_envelope_type(
                "pete/motion-safety-observation@1",
                record(
                    "pete/motion-safety-condition@1",
                    vec![
                        field("inhibited", leaf("value/bool")),
                        field("motion_available", leaf("value/bool")),
                    ],
                ),
            ),
        ),
        (
            "PeteImportantCapabilityObservation",
            observation_envelope_type(
                "pete/important-capability-observation@1",
                record(
                    "observation/capability-availability@1",
                    vec![field("available", leaf("value/bool"))],
                ),
            ),
        ),
    ]
}

pub fn install_homeostasis_catalogs(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    let types = homeostasis_registered_types();
    for (name, value_type) in &types {
        startup
            .insert_structured_type(*name, value_type.clone())
            .map_err(|error| error.to_string())?;
    }
    // The output deliberately uses the existing body-input waist consumed by
    // CurrentExperience. Homeostasis does not create a second experience path.
    let output = conduit_semantic_catalog::experience_body_input_type();
    startup
        .insert(KindSignature {
            kind: HOMEOSTASIS_REDUCE_KIND.into(),
            startup_parameters: policy_startup_parameters(),
        })
        .map_err(|error| error.to_string())?;
    profile
        .insert(KindProjection {
            kind_id: kind_id(HOMEOSTASIS_REDUCE_KIND),
            kind_contract_revision: KindIdentity::from(HOMEOSTASIS_REDUCE_REVISION),
            inputs: types
                .iter()
                .map(|(name, value_type)| port(input_port(name), value_type, PortDirection::Input))
                .collect(),
            outputs: vec![port("state", &output, PortDirection::Output)],
            configuration: policy_configuration(),
        })
        .map_err(|error| error.to_string())
}

fn policy_startup_parameters() -> Vec<StartupParameterSignature> {
    vec![
        parameter(
            "policy-revision",
            "Text",
            "\"conduit.pete/homeostasis-thresholds@1\"",
        ),
        parameter("energy-low-permille", "Count", "250"),
        parameter("energy-critical-permille", "Count", "100"),
        parameter("thermal-constrained-milli-celsius", "Scalar", "75000"),
        parameter("thermal-critical-milli-celsius", "Scalar", "90000"),
        parameter("pressure-high-permille", "Count", "800"),
        parameter("pressure-critical-permille", "Count", "950"),
    ]
}

fn parameter(name: &str, value_type: &str, default: &str) -> StartupParameterSignature {
    StartupParameterSignature {
        name: name.into(),
        value_type: value_type.into(),
        default: Some(default.into()),
    }
}

fn policy_configuration() -> Vec<KindConfigurationField> {
    vec![
        text_policy("policy-revision", "conduit.pete/homeostasis-thresholds@1"),
        count_policy("energy-low-permille", 250, 1_000),
        count_policy("energy-critical-permille", 100, 1_000),
        scalar_policy("thermal-constrained-milli-celsius", 75_000),
        scalar_policy("thermal-critical-milli-celsius", 90_000),
        count_policy("pressure-high-permille", 800, 1_000),
        count_policy("pressure-critical-permille", 950, 1_000),
    ]
}

fn text_policy(key: &str, value: &str) -> KindConfigurationField {
    KindConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::Text(value.into()),
        rule: KindConfigurationRule::TextOneOf {
            values: vec![value.into()],
        },
    }
}

fn count_policy(key: &str, value: u64, maximum: u64) -> KindConfigurationField {
    KindConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::U64(value),
        rule: KindConfigurationRule::U64Range {
            minimum: 0,
            maximum,
        },
    }
}

fn scalar_policy(key: &str, value: i64) -> KindConfigurationField {
    KindConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::I64(value),
        rule: KindConfigurationRule::I64Range {
            minimum: -273_150,
            maximum: 1_000_000,
        },
    }
}

fn input_port(type_name: &str) -> &'static str {
    match type_name {
        "PetePowerObservation" => "power",
        "PeteThermalObservation" => "thermal",
        "PeteComputePressureObservation" => "compute_pressure",
        "PeteStoragePressureObservation" => "storage_pressure",
        "PeteMotionSafetyObservation" => "motion_safety",
        "PeteImportantCapabilityObservation" => "important_capability",
        _ => unreachable!("reviewed homeostasis type"),
    }
}

fn port(name: &str, value_type: &StructuredInfoType, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: value_type
            .profile()
            .expect("reviewed homeostasis profile")
            .value_kind()
            .clone(),
        direction,
        temporal: if direction == PortDirection::Input {
            PortTemporal::Flow { closes: false }
        } else {
            PortTemporal::Current
        },
    }
}
