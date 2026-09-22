//! Authoring catalog for the ordinary reusable Homeostasis form.

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, KindIdentity, PortDescriptor, PortDirection, PortTemporal,
    StructuredFieldType, StructuredInfoType,
};
use conduit_form::{KindProjection, KindSignature, ProfileCatalog, StartupCatalog};

pub const HOMEOSTASIS_REDUCE_KIND: &str = "experience/reduce-homeostasis";
pub const HOMEOSTASIS_REDUCE_REVISION: &str = "conduit.homeostasis/reduce@1";

fn leaf(kind: &str) -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(kind)).expect("reviewed homeostasis leaf")
}

fn field(name: &str, value_type: StructuredInfoType) -> StructuredFieldType {
    StructuredFieldType::new(name, value_type).expect("reviewed homeostasis field")
}

fn input_type(kind: &str) -> StructuredInfoType {
    StructuredInfoType::record(
        kind_id(kind),
        vec![
            field("source_identity", leaf("value/text")),
            field("observed_at", leaf("value/text")),
            field("availability", leaf("value/text")),
            field("value", leaf("value/text")),
            field("uncertainty_permille", leaf("value/count")),
            field("calibration_profile_identity", leaf("value/text")),
        ],
    )
    .expect("reviewed bounded homeostasis input")
}

pub fn homeostasis_registered_types() -> Vec<(&'static str, StructuredInfoType)> {
    vec![
        (
            "HomeostaticPowerInput",
            input_type("experience/homeostatic-power-input@1"),
        ),
        (
            "HomeostaticThermalInput",
            input_type("experience/homeostatic-thermal-input@1"),
        ),
        (
            "HomeostaticComputePressureInput",
            input_type("experience/homeostatic-compute-pressure-input@1"),
        ),
        (
            "HomeostaticStoragePressureInput",
            input_type("experience/homeostatic-storage-pressure-input@1"),
        ),
        (
            "HomeostaticSafetyInput",
            input_type("experience/homeostatic-safety-input@1"),
        ),
        (
            "HomeostaticCapabilityInput",
            input_type("experience/homeostatic-capability-input@1"),
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
    let output = crate::experience_body_input_type();
    startup
        .insert(KindSignature {
            kind: HOMEOSTASIS_REDUCE_KIND.into(),
            startup_parameters: vec![],
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
            configuration: vec![],
        })
        .map_err(|error| error.to_string())
}

fn input_port(type_name: &str) -> &'static str {
    match type_name {
        "HomeostaticPowerInput" => "power",
        "HomeostaticThermalInput" => "thermal",
        "HomeostaticComputePressureInput" => "compute_pressure",
        "HomeostaticStoragePressureInput" => "storage_pressure",
        "HomeostaticSafetyInput" => "motion_safety",
        "HomeostaticCapabilityInput" => "important_capability",
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
