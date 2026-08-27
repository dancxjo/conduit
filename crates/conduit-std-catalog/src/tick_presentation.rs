use super::{
    StandardConfigurationField, StandardConfigurationRule, StandardKindContract, TerminalBehavior,
};
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    kind_id, port_id, present_host_operation_requirement, resource_requirement, ArtifactId,
    CapabilityId, CapabilityLimits, CapabilityOffer, ConfigurationValue, ExecutionProfileId,
    ImplementationId, KindContractRevision, PortDescriptor, PortDirection,
    PRESENTATION_RESOURCE_CLASS,
};

pub const TICK_PRESENTATION_KIND: &str = "presentation/tick";
pub const TICK_PRESENTATION_CONTRACT_REVISION: &str = "conduit.std/presentation-tick@1";
pub const TICK_PRESENTATION_EXECUTION_PROFILE: &str =
    "conduit.std/presentation-tick-kernel-hosted@1";
pub const TICK_PRESENTATION_IMPLEMENTATION: &str = "std/kernel-presentation-tick@1";
pub const TICK_PRESENTATION_ARTIFACT: &str = "conduit-std-host/presentation-tick@1";
pub const TICK_PRESENTATION_CAPABILITY: &str = "presentation-tick-v1";
pub const TICK_PRESENTATION_TARGET: &str = "presentation/stdout-tick";

pub fn tick_presentation_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(TICK_PRESENTATION_KIND),
        plain_name: "Tick presentation".to_string(),
        summary: "Present up to four exact typed tick sequence values on stdout.".to_string(),
        inputs: vec![PortDescriptor {
            port_id: port_id("tick"),
            value_kind: kind_id(conduit_time::TICK_VALUE_KIND),
            direction: PortDirection::Input,
            temporal: conduit_core::PortTemporal::Flow { closes: true },
        }],
        outputs: Vec::new(),
        configuration: vec![StandardConfigurationField {
            key: "maximum-values".to_string(),
            default_value: ConfigurationValue::U64(conduit_time::TIME_EVERY_COUNT),
            rule: StandardConfigurationRule::U64Range {
                minimum: 1,
                maximum: conduit_time::TIME_EVERY_COUNT,
            },
        }],
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: conduit_time::TIME_EVERY_COUNT as u16,
            max_queue_bytes: 64,
        },
        terminal_behavior: TerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "show: presentation/tick".to_string(),
    }
}

pub fn tick_presentation_offer() -> CapabilityOffer {
    let contract = tick_presentation_contract();
    CapabilityOffer {
        startup_parameters: vec![conduit_core::FaceStartupParameter {
            name: "maximum-values".to_string(),
            value_type: "Count".to_string(),
            has_default: true,
        }],
        shorthand: None,
        capability_id: CapabilityId::from(TICK_PRESENTATION_CAPABILITY),
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(TICK_PRESENTATION_CONTRACT_REVISION),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(TICK_PRESENTATION_EXECUTION_PROFILE),
            implementation_id: ImplementationId::from(TICK_PRESENTATION_IMPLEMENTATION),
            artifact_id: ArtifactId::from(TICK_PRESENTATION_ARTIFACT),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![present_host_operation_requirement(
            kind_id(TICK_PRESENTATION_TARGET),
            conduit_time::TICK_ENCODED_LEN,
        )],
        resource_requirements: vec![resource_requirement(PRESENTATION_RESOURCE_CLASS, 1)],
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

#[cfg(feature = "form-catalog")]
pub fn tick_presentation_kind_definition() -> conduit_form::KindDefinition {
    use conduit_form::{ConfigurationField, ConfigurationRule, KindDefinition};
    let contract = tick_presentation_contract();
    KindDefinition {
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(TICK_PRESENTATION_CONTRACT_REVISION),
        inputs: contract.inputs,
        outputs: contract.outputs,
        configuration: vec![ConfigurationField {
            key: "maximum-values".to_string(),
            default_value: ConfigurationValue::U64(conduit_time::TIME_EVERY_COUNT),
            validation: ConfigurationRule::U64Range {
                minimum: 1,
                maximum: conduit_time::TIME_EVERY_COUNT,
            },
        }],
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_tick_presentation_catalog(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_form::{KindSignature, StartupParameterSignature};
    startup.insert(KindSignature {
        kind: TICK_PRESENTATION_KIND.to_string(),
        startup_parameters: vec![StartupParameterSignature {
            name: "maximum-values".to_string(),
            value_type: "Count".to_string(),
            default: Some(conduit_time::TIME_EVERY_COUNT.to_string()),
        }],
    })?;
    profile
        .insert(tick_presentation_kind_definition())
        .map_err(|error| error.to_string())
}
