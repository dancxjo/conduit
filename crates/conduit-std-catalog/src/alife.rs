//! Portable bounded Lenia meanings and truthful std-host realization offers.

use super::{
    StandardConfigurationField, StandardConfigurationRule, StandardKindContract, TerminalBehavior,
};
use alloc::{string::ToString, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, present_host_operation_requirement, resource_requirement, ArtifactId,
    CapabilityId, CapabilityLimits, CapabilityOffer, ConfigurationValue, ExecutionProfileId,
    HostOperationContractId, HostOperationRequirement, ImplementationId, KindContractRevision,
    PortDescriptor, PortDirection, PortTemporal, LENIA_MAXIMUM_FIELD_BYTES,
    LENIA_MAXIMUM_KERNEL_RADIUS, PRESENTATION_RESOURCE_CLASS, SCALAR_FIELD2_INFO_ID,
};

pub const ORBIUM_SEED_KIND: &str = "alife/orbium-seed";
pub const LENIA_STEP_KIND: &str = "alife/lenia-step";
pub const SCALAR_FIELD_PRESENTATION_KIND: &str = "presentation/scalar-field";

pub const ORBIUM_SEED_REVISION: &str = "conduit.alife/orbium-seed@1";
pub const LENIA_STEP_REVISION: &str = "conduit.alife/lenia-step@1";
pub const SCALAR_FIELD_PRESENTATION_REVISION: &str = "conduit.presentation/scalar-field@1";

pub const ORBIUM_SEED_EXECUTION_PROFILE: &str = "conduit.std/orbium-seed-fixed-q16.16@1";
pub const LENIA_STEP_EXECUTION_PROFILE: &str = "conduit.std/lenia-spatial-fixed-q16.16@1";
pub const SCALAR_FIELD_PRESENTATION_EXECUTION_PROFILE: &str =
    "conduit.std/present-scalar-field-terminal@1";

pub const ORBIUM_SEED_IMPLEMENTATION: &str = "std/kernel-orbium-seed@1";
pub const LENIA_STEP_IMPLEMENTATION: &str = "std/kernel-lenia-step@1";
pub const SCALAR_FIELD_PRESENTATION_IMPLEMENTATION: &str = "std/kernel-present-scalar-field@1";

pub const ORBIUM_SEED_ARTIFACT: &str = "conduit-std-host/orbium-seed@1";
pub const LENIA_STEP_ARTIFACT: &str = "conduit-std-host/lenia-spatial-q16@1";
pub const SCALAR_FIELD_PRESENTATION_ARTIFACT: &str = "conduit-std-host/presentation-scalar-field@1";

pub const LENIA_INITIALIZE_HOST_OPERATION: &str = "conduit.host/lenia-initialize@1";
pub const LENIA_STEP_HOST_OPERATION: &str = "conduit.host/lenia-step@1";
pub const SCALAR_FIELD_PRESENTATION_TARGET: &str = "presentation/stdout-scalar-field";

pub const ORBIUM_WIDTH_KEY: &str = "width";
pub const ORBIUM_HEIGHT_KEY: &str = "height";
pub const SEED_KEY: &str = "seed";
pub const KERNEL_RADIUS_KEY: &str = "kernel_radius";
pub const KERNEL_MU_KEY: &str = "kernel_mu";
pub const KERNEL_SIGMA_KEY: &str = "kernel_sigma";
pub const GROWTH_MU_KEY: &str = "growth_mu";
pub const GROWTH_SIGMA_KEY: &str = "growth_sigma";
pub const DT_KEY: &str = "dt";
pub const BOUNDARY_KEY: &str = "boundary";
pub const NUMERIC_PROFILE_KEY: &str = "numeric_profile";
pub const TITLE_KEY: &str = "title";
pub const MINIMUM_KEY: &str = "minimum";
pub const MAXIMUM_KEY: &str = "maximum";
pub const MAXIMUM_PRESENTED_FIELDS: u16 = 4;

pub fn alife_contracts() -> Vec<StandardKindContract> {
    vec![
        orbium_seed_contract(),
        lenia_step_contract(),
        scalar_field_presentation_contract(),
    ]
}

pub fn alife_offers() -> Vec<CapabilityOffer> {
    vec![
        orbium_seed_offer(),
        lenia_step_offer(),
        scalar_field_presentation_offer(),
    ]
}

pub fn orbium_seed_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(ORBIUM_SEED_KIND),
        plain_name: "Deterministic Orbium seed".to_string(),
        summary: "Construct one bounded portable ScalarField2 specimen from semantic dimensions and seed.".to_string(),
        inputs: Vec::new(),
        outputs: vec![field_port("field", PortDirection::Output, PortTemporal::Value)],
        configuration: vec![
            u64_field(ORBIUM_WIDTH_KEY, 128, 32, 128),
            u64_field(ORBIUM_HEIGHT_KEY, 128, 32, 128),
            u64_field(SEED_KEY, 1, 0, u64::MAX),
        ],
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: 4,
            max_queue_bytes: LENIA_MAXIMUM_FIELD_BYTES * 4,
        },
        terminal_behavior: TerminalBehavior::EmitsOneField,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "seed: alife/orbium-seed(width = 128, height = 128, seed = 1)".to_string(),
    }
}

pub fn lenia_step_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(LENIA_STEP_KIND),
        plain_name: "Lenia field evolution".to_string(),
        summary: "Evolve an initialized ScalarField2 once per closing-flow Tick using exact fixed-Q16.16 Lenia semantics.".to_string(),
        inputs: vec![
            field_port("initial", PortDirection::Input, PortTemporal::Value),
            tick_port("tick", PortDirection::Input),
        ],
        outputs: vec![field_port(
            "field",
            PortDirection::Output,
            PortTemporal::Flow { closes: true },
        )],
        configuration: vec![
            u64_field(
                KERNEL_RADIUS_KEY,
                13,
                1,
                u64::from(LENIA_MAXIMUM_KERNEL_RADIUS),
            ),
            scalar_field(KERNEL_MU_KEY, 500_000, 0, 1_000_000),
            scalar_field(KERNEL_SIGMA_KEY, 150_000, 1, 1_000_000),
            scalar_field(GROWTH_MU_KEY, 150_000, 0, 1_000_000),
            scalar_field(GROWTH_SIGMA_KEY, 15_000, 1, 1_000_000),
            scalar_field(DT_KEY, 100_000, 1, 1_000_000),
            text_choice(BOUNDARY_KEY, "wrap", &["wrap"]),
            text_choice(
                NUMERIC_PROFILE_KEY,
                "fixed-q16.16",
                &["fixed-q16.16"],
            ),
        ],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: MAXIMUM_PRESENTED_FIELDS + 1,
            max_queue_bytes: LENIA_MAXIMUM_FIELD_BYTES + 64,
        },
        terminal_behavior: TerminalBehavior::EvolvesAfterTicksAndCompletesWhenTickCloses,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "evolve: alife/lenia-step(kernel_radius = 13, growth_mu = 0.15)".to_string(),
    }
}

pub fn scalar_field_presentation_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(SCALAR_FIELD_PRESENTATION_KIND),
        plain_name: "Scalar field presentation".to_string(),
        summary:
            "Manifest each bounded ScalarField2 through one exact admitted presentation effect."
                .to_string(),
        inputs: vec![field_port(
            "field",
            PortDirection::Input,
            PortTemporal::Flow { closes: true },
        )],
        outputs: Vec::new(),
        configuration: vec![
            StandardConfigurationField {
                key: TITLE_KEY.to_string(),
                default_value: ConfigurationValue::Text("Scalar field".to_string()),
                rule: StandardConfigurationRule::TextBytes { maximum: 64 },
            },
            scalar_field(MINIMUM_KEY, 0, 0, 1_000_000),
            scalar_field(MAXIMUM_KEY, 1_000_000, 0, 1_000_000),
        ],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: MAXIMUM_PRESENTED_FIELDS,
            max_queue_bytes: LENIA_MAXIMUM_FIELD_BYTES * u32::from(MAXIMUM_PRESENTED_FIELDS),
        },
        terminal_behavior: TerminalBehavior::PresentsEachFieldAndCompletesWhenInputCloses,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "show: presentation/scalar-field(title = \"Orbium\", minimum = 0, maximum = 1)"
            .to_string(),
    }
}

pub fn orbium_seed_offer() -> CapabilityOffer {
    offer(
        orbium_seed_contract(),
        OfferIdentity {
            capability: "std-orbium-seed-v1",
            revision: ORBIUM_SEED_REVISION,
            profile: ORBIUM_SEED_EXECUTION_PROFILE,
            implementation: ORBIUM_SEED_IMPLEMENTATION,
            artifact: ORBIUM_SEED_ARTIFACT,
        },
        Vec::new(),
        Vec::new(),
    )
}

pub fn lenia_step_offer() -> CapabilityOffer {
    offer(
        lenia_step_contract(),
        OfferIdentity {
            capability: "std-lenia-step-v1",
            revision: LENIA_STEP_REVISION,
            profile: LENIA_STEP_EXECUTION_PROFILE,
            implementation: LENIA_STEP_IMPLEMENTATION,
            artifact: LENIA_STEP_ARTIFACT,
        },
        vec![
            HostOperationRequirement {
                contract_id: HostOperationContractId::from(LENIA_INITIALIZE_HOST_OPERATION),
                target_kind: Some(kind_id(LENIA_STEP_KIND)),
                maximum_in_flight: 1,
                maximum_input_bytes: LENIA_MAXIMUM_FIELD_BYTES,
                maximum_output_bytes: 0,
            },
            HostOperationRequirement {
                contract_id: HostOperationContractId::from(LENIA_STEP_HOST_OPERATION),
                target_kind: Some(kind_id(LENIA_STEP_KIND)),
                maximum_in_flight: 1,
                maximum_input_bytes: super::TICK_ENCODED_LEN,
                maximum_output_bytes: LENIA_MAXIMUM_FIELD_BYTES,
            },
        ],
        Vec::new(),
    )
}

pub fn scalar_field_presentation_offer() -> CapabilityOffer {
    offer(
        scalar_field_presentation_contract(),
        OfferIdentity {
            capability: "std-scalar-field-presentation-v1",
            revision: SCALAR_FIELD_PRESENTATION_REVISION,
            profile: SCALAR_FIELD_PRESENTATION_EXECUTION_PROFILE,
            implementation: SCALAR_FIELD_PRESENTATION_IMPLEMENTATION,
            artifact: SCALAR_FIELD_PRESENTATION_ARTIFACT,
        },
        vec![present_host_operation_requirement(
            kind_id(SCALAR_FIELD_PRESENTATION_TARGET),
            LENIA_MAXIMUM_FIELD_BYTES,
        )],
        vec![resource_requirement(PRESENTATION_RESOURCE_CLASS, 1)],
    )
}

struct OfferIdentity<'a> {
    capability: &'a str,
    revision: &'a str,
    profile: &'a str,
    implementation: &'a str,
    artifact: &'a str,
}

fn offer(
    contract: StandardKindContract,
    identity: OfferIdentity<'_>,
    host_operations: Vec<HostOperationRequirement>,
    resource_requirements: Vec<conduit_core::ResourceRequirement>,
) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: super::startup_face(&contract.configuration),
        shorthand: None,
        capability_id: CapabilityId::from(identity.capability),
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(identity.revision),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(identity.profile),
            implementation_id: ImplementationId::from(identity.implementation),
            artifact_id: ArtifactId::from(identity.artifact),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations,
        resource_requirements,
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

fn field_port(name: &str, direction: PortDirection, temporal: PortTemporal) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(SCALAR_FIELD2_INFO_ID),
        direction,
        temporal,
    }
}

fn tick_port(name: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(super::TICK_VALUE_KIND),
        direction,
        temporal: PortTemporal::Flow { closes: true },
    }
}

fn u64_field(key: &str, default: u64, minimum: u64, maximum: u64) -> StandardConfigurationField {
    StandardConfigurationField {
        key: key.to_string(),
        default_value: ConfigurationValue::U64(default),
        rule: StandardConfigurationRule::U64Range { minimum, maximum },
    }
}

fn scalar_field(key: &str, default: i64, minimum: i64, maximum: i64) -> StandardConfigurationField {
    StandardConfigurationField {
        key: key.to_string(),
        default_value: ConfigurationValue::I64(default),
        rule: StandardConfigurationRule::I64Range { minimum, maximum },
    }
}

fn text_choice(key: &str, default: &str, values: &[&str]) -> StandardConfigurationField {
    StandardConfigurationField {
        key: key.to_string(),
        default_value: ConfigurationValue::Text(default.to_string()),
        rule: StandardConfigurationRule::TextOneOf {
            values: values.iter().map(|value| (*value).to_string()).collect(),
        },
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_alife_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_form::{ConfigurationField, ConfigurationRule, KindDefinition, KindSignature};
    for (contract, revision) in [
        (orbium_seed_contract(), ORBIUM_SEED_REVISION),
        (lenia_step_contract(), LENIA_STEP_REVISION),
        (
            scalar_field_presentation_contract(),
            SCALAR_FIELD_PRESENTATION_REVISION,
        ),
    ] {
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().to_string(),
            startup_parameters: contract
                .configuration
                .iter()
                .map(|field| conduit_form::StartupParameterSignature {
                    name: field.key.clone(),
                    value_type: match field.default_value {
                        ConfigurationValue::U64(_) => "Count",
                        ConfigurationValue::I64(_) => "Scalar",
                        ConfigurationValue::Text(_) => "Text",
                        ConfigurationValue::Bool(_) => "Boolean",
                        ConfigurationValue::Structured(_) => "Structured",
                    }
                    .to_string(),
                    default: Some(render_default(&field.default_value)),
                })
                .collect(),
        })?;
        profile
            .insert(KindDefinition {
                kind_id: contract.kind_id,
                kind_contract_revision: KindContractRevision::from(revision),
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration: contract
                    .configuration
                    .into_iter()
                    .map(|field| ConfigurationField {
                        key: field.key,
                        default_value: field.default_value,
                        validation: match field.rule {
                            StandardConfigurationRule::Any => ConfigurationRule::Any,
                            StandardConfigurationRule::U64Range { minimum, maximum } => {
                                ConfigurationRule::U64Range { minimum, maximum }
                            }
                            StandardConfigurationRule::I64Range { minimum, maximum } => {
                                ConfigurationRule::I64Range { minimum, maximum }
                            }
                            StandardConfigurationRule::DurationMillis { minimum, maximum } => {
                                ConfigurationRule::DurationMillis { minimum, maximum }
                            }
                            StandardConfigurationRule::TextBytes { maximum } => {
                                ConfigurationRule::TextBytes { maximum }
                            }
                            StandardConfigurationRule::TextOneOf { values } => {
                                ConfigurationRule::TextOneOf { values }
                            }
                        },
                    })
                    .collect(),
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(feature = "form-catalog")]
fn render_default(value: &ConfigurationValue) -> alloc::string::String {
    match value {
        ConfigurationValue::Bool(value) => value.to_string(),
        ConfigurationValue::U64(value) => value.to_string(),
        ConfigurationValue::I64(value) => render_scalar(*value),
        ConfigurationValue::Text(value) => alloc::format!("\"{value}\""),
        ConfigurationValue::Structured(_) => "structured".to_string(),
    }
}

#[cfg(feature = "form-catalog")]
fn render_scalar(value: i64) -> alloc::string::String {
    let negative = value < 0;
    let magnitude = value.unsigned_abs();
    let whole = magnitude / conduit_core::Scalar::SCALE as u64;
    let fraction = magnitude % conduit_core::Scalar::SCALE as u64;
    let sign = if negative { "-" } else { "" };
    if fraction == 0 {
        alloc::format!("{sign}{whole}.0")
    } else {
        let mut fraction = alloc::format!("{fraction:06}");
        while fraction.ends_with('0') {
            fraction.pop();
        }
        alloc::format!("{sign}{whole}.{fraction}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contracts_are_exact_finite_and_platform_neutral() {
        let contracts = alife_contracts();
        let offers = alife_offers();
        assert_eq!(contracts.len(), offers.len());
        for (contract, offer) in contracts.iter().zip(&offers) {
            assert_eq!(contract.kind_id, offer.kind_id);
            assert_eq!(contract.inputs, offer.inputs);
            assert_eq!(contract.outputs, offer.outputs);
            assert_eq!(contract.limits, offer.limits);
        }
        let portable = alloc::format!("{contracts:?}").to_ascii_lowercase();
        for forbidden in ["host/", "boot/", "websocket", "framebuffer", "dom", "gpio"] {
            assert!(!portable.contains(forbidden), "leaked {forbidden}");
        }
    }
}
