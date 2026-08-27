//! Portable bounded Lenia meanings and truthful std-host realization offers.

use super::{
    StandardConfigurationField, StandardConfigurationRule, StandardKindContract, TerminalBehavior,
};
use alloc::{string::ToString, vec, vec::Vec};
use conduit_alife::{
    LENIA_MAXIMUM_FIELD_BYTES, LENIA_STEP_KIND, LENIA_STEP_REVISION, MAXIMUM_PRESENTED_FIELDS,
    ORBIUM_SEED_REVISION, SCALAR_FIELD_PRESENTATION_REVISION,
};
use conduit_core::{
    kind_id, present_host_operation_requirement, resource_requirement, ArtifactId, CapabilityId,
    CapabilityLimits, CapabilityOffer, ExecutionProfileId, HostOperationContractId,
    HostOperationRequirement, ImplementationId, KindContractRevision, PRESENTATION_RESOURCE_CLASS,
};

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
    let definition = conduit_alife::orbium_seed_definition();
    StandardKindContract {
        kind_id: definition.kind_id,
        plain_name: "Deterministic Orbium seed".to_string(),
        summary: "Construct one bounded portable ScalarField2 specimen from semantic dimensions and seed.".to_string(),
        inputs: definition.inputs,
        outputs: definition.outputs,
        configuration: standard_configuration(definition.configuration),
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
    let definition = conduit_alife::lenia_step_definition();
    StandardKindContract {
        kind_id: definition.kind_id,
        plain_name: "Lenia field evolution".to_string(),
        summary: "Evolve an initialized ScalarField2 once per closing-flow Tick using exact fixed-Q16.16 Lenia semantics.".to_string(),
        inputs: definition.inputs,
        outputs: definition.outputs,
        configuration: standard_configuration(definition.configuration),
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
    let definition = conduit_alife::scalar_field_presentation_definition();
    StandardKindContract {
        kind_id: definition.kind_id,
        plain_name: "Scalar field presentation".to_string(),
        summary:
            "Manifest each bounded ScalarField2 through one exact admitted presentation effect."
                .to_string(),
        inputs: definition.inputs,
        outputs: definition.outputs,
        configuration: standard_configuration(definition.configuration),
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
                maximum_input_bytes: conduit_time::TICK_ENCODED_LEN,
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

fn standard_configuration(
    fields: Vec<conduit_form::ConfigurationField>,
) -> Vec<StandardConfigurationField> {
    fields
        .into_iter()
        .map(|field| StandardConfigurationField {
            key: field.key,
            default_value: field.default_value,
            rule: match field.validation {
                conduit_form::ConfigurationRule::Any => StandardConfigurationRule::Any,
                conduit_form::ConfigurationRule::U64Range { minimum, maximum } => {
                    StandardConfigurationRule::U64Range { minimum, maximum }
                }
                conduit_form::ConfigurationRule::I64Range { minimum, maximum } => {
                    StandardConfigurationRule::I64Range { minimum, maximum }
                }
                conduit_form::ConfigurationRule::DurationMillis { minimum, maximum } => {
                    StandardConfigurationRule::DurationMillis { minimum, maximum }
                }
                conduit_form::ConfigurationRule::TextBytes { maximum } => {
                    StandardConfigurationRule::TextBytes { maximum }
                }
                conduit_form::ConfigurationRule::TextOneOf { values } => {
                    StandardConfigurationRule::TextOneOf { values }
                }
                conduit_form::ConfigurationRule::Structured { .. } => {
                    unreachable!("Lenia definitions do not use structured configuration")
                }
            },
        })
        .collect()
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
