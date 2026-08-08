use super::{
    StandardConfigurationField, StandardConfigurationRule, StandardKindContract, TerminalBehavior,
};
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    kind_id, port_id, present_host_operation_requirement, resource_requirement, ArtifactId,
    CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId, ImplementationId,
    KindContractRevision, PortDescriptor, PortDirection, PRESENTATION_RESOURCE_CLASS,
};

pub const TEXT_PRESENTATION_KIND: &str = "presentation/text";
pub const TEXT_PRESENTATION_VALUE_KIND: &str = "value/text@1";
pub const TEXT_PRESENTATION_CONTRACT_REVISION: &str = "conduit.std/presentation-text@1";
pub const TEXT_PRESENTATION_EXECUTION_PROFILE: &str =
    "conduit.std/presentation-text-kernel-hosted@1";
pub const TEXT_PRESENTATION_IMPLEMENTATION: &str = "std/kernel-presentation-text@1";
pub const TEXT_PRESENTATION_ARTIFACT: &str = "conduit-std-host/presentation-text@1";
pub const TEXT_PRESENTATION_CAPABILITY: &str = "presentation-text-v1";
pub const MAX_TEXT_BYTES: u32 = 256;
pub const MAX_TEXT_VALUES: u64 = 4;

pub fn text_presentation_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(TEXT_PRESENTATION_KIND),
        plain_name: "Text presentation".to_string(),
        summary: "Present up to four bounded UTF-8 text values on the host's text surface."
            .to_string(),
        inputs: text_presentation_inputs(),
        outputs: Vec::new(),
        configuration: vec![StandardConfigurationField {
            key: "maximum-values".to_string(),
            default_value: conduit_core::ConfigurationValue::U64(MAX_TEXT_VALUES),
            rule: StandardConfigurationRule::U64Range {
                minimum: 1,
                maximum: MAX_TEXT_VALUES,
            },
        }],
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 4,
            max_queue_bytes: MAX_TEXT_BYTES,
        },
        terminal_behavior: TerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "show: presentation/text".to_string(),
    }
}

pub fn text_presentation_inputs() -> Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id("text"),
        value_kind: kind_id(TEXT_PRESENTATION_VALUE_KIND),
        direction: PortDirection::Input,
        temporal: conduit_core::PortTemporal::Value,
    }]
}

pub fn text_presentation_offer() -> CapabilityOffer {
    let contract = text_presentation_contract();
    CapabilityOffer {
        startup_parameters: vec![conduit_core::FaceStartupParameter {
            name: "maximum-values".to_string(),
            value_type: "Count".to_string(),
            has_default: true,
        }],
        shorthand: None,
        capability_id: CapabilityId::from(TEXT_PRESENTATION_CAPABILITY),
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(TEXT_PRESENTATION_CONTRACT_REVISION),
        execution_profile_id: ExecutionProfileId::from(TEXT_PRESENTATION_EXECUTION_PROFILE),
        implementation_id: ImplementationId::from(TEXT_PRESENTATION_IMPLEMENTATION),
        artifact_id: ArtifactId::from(TEXT_PRESENTATION_ARTIFACT),
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![present_host_operation_requirement(
            kind_id("presentation/stdout-text"),
            MAX_TEXT_BYTES,
        )],
        resource_requirements: vec![resource_requirement(PRESENTATION_RESOURCE_CLASS, 1)],
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

#[cfg(feature = "form-catalog")]
pub fn text_presentation_profile_catalog() -> conduit_form::ProfileCatalog {
    use conduit_form::{ConfigurationField, ConfigurationRule, KindDefinition, ProfileCatalog};
    let mut catalog = ProfileCatalog::new();
    catalog
        .insert(KindDefinition {
            kind_id: kind_id(TEXT_PRESENTATION_KIND),
            kind_contract_revision: KindContractRevision::from(TEXT_PRESENTATION_CONTRACT_REVISION),
            inputs: text_presentation_inputs(),
            outputs: Vec::new(),
            configuration: vec![ConfigurationField {
                key: "maximum-values".to_string(),
                default_value: conduit_core::ConfigurationValue::U64(MAX_TEXT_VALUES),
                validation: ConfigurationRule::U64Range {
                    minimum: 1,
                    maximum: MAX_TEXT_VALUES,
                },
            }],
        })
        .expect("the one-kind text presentation catalog is unique");
    catalog
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_text_presentation_contract_is_exact_and_hosted_only() {
        let contract = text_presentation_contract();
        let offer = text_presentation_offer();
        assert_eq!(
            contract.inputs[0].value_kind.as_str(),
            TEXT_PRESENTATION_VALUE_KIND
        );
        assert_eq!(offer.inputs, contract.inputs);
        assert_eq!(
            offer.implementation_id.as_str(),
            TEXT_PRESENTATION_IMPLEMENTATION
        );
        assert_eq!(offer.host_operations[0].maximum_input_bytes, MAX_TEXT_BYTES);
        assert!(!contract.browser_manifestation_honest && !contract.pico_manifestation_honest);
    }
}
