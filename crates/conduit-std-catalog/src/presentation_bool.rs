//! Portable Boolean presentation meaning and the accepted browser implementation offer.

use super::{StandardKindContract, TerminalBehavior};
#[cfg(feature = "form-catalog")]
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    kind_id, port_id, present_host_operation_requirement, resource_requirement, ArtifactId,
    CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId, ImplementationId,
    KindContractRevision, PortDescriptor, PortDirection, PortTemporal, BOOL_INFO_ID,
    PRESENTATION_RESOURCE_CLASS,
};

pub const BOOL_PRESENTATION_KIND: &str = "presentation/bool";
pub const BOOL_PRESENTATION_CONTRACT_REVISION: &str = "conduit.presentation/bool@1";
pub const BOOL_PRESENTATION_EXECUTION_PROFILE: &str = "conduit.browser/present-bool@1";
pub const BOOL_PRESENTATION_IMPLEMENTATION: &str = "browser/kernel-dom-show-bool@1";
pub const BOOL_PRESENTATION_ARTIFACT: &str = "conduit-browser-runtime/show-bool@1";
pub const BOOL_PRESENTATION_CAPABILITY: &str = "browser-bool-presentation-v1";
pub const BOOL_PRESENTATION_TARGET: &str = "presentation/browser-bool";
pub const BOOL_PRESENTATION_STD_EXECUTION_PROFILE: &str =
    "conduit.std/present-bool-kernel-hosted@1";
pub const BOOL_PRESENTATION_STD_IMPLEMENTATION: &str = "std/kernel-present-bool@1";
pub const BOOL_PRESENTATION_STD_ARTIFACT: &str = "conduit-std-host/present-bool@1";
pub const BOOL_PRESENTATION_STD_CAPABILITY: &str = "std-bool-presentation-v1";
pub const BOOL_PRESENTATION_STD_TARGET: &str = "presentation/stdout-bool";

pub fn bool_presentation_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(BOOL_PRESENTATION_KIND),
        plain_name: "Present current Boolean".to_string(),
        summary: "Manifest each current Boolean through an admitted presenter effect.".to_string(),
        inputs: vec![PortDescriptor {
            port_id: port_id("value"),
            value_kind: kind_id(BOOL_INFO_ID),
            direction: PortDirection::Input,
            temporal: PortTemporal::Current,
        }],
        outputs: Vec::new(),
        configuration: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 8,
        },
        terminal_behavior: TerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: true,
        pico_manifestation_honest: false,
        example: "show: presentation/bool".to_string(),
    }
}

pub fn bool_presentation_browser_offer() -> CapabilityOffer {
    bool_presentation_offer(
        BOOL_PRESENTATION_CAPABILITY,
        BOOL_PRESENTATION_EXECUTION_PROFILE,
        BOOL_PRESENTATION_IMPLEMENTATION,
        BOOL_PRESENTATION_ARTIFACT,
        BOOL_PRESENTATION_TARGET,
    )
}

pub fn bool_presentation_std_offer() -> CapabilityOffer {
    bool_presentation_offer(
        BOOL_PRESENTATION_STD_CAPABILITY,
        BOOL_PRESENTATION_STD_EXECUTION_PROFILE,
        BOOL_PRESENTATION_STD_IMPLEMENTATION,
        BOOL_PRESENTATION_STD_ARTIFACT,
        BOOL_PRESENTATION_STD_TARGET,
    )
}

fn bool_presentation_offer(
    capability: &'static str,
    execution_profile: &'static str,
    implementation: &'static str,
    artifact: &'static str,
    target: &'static str,
) -> CapabilityOffer {
    let contract = bool_presentation_contract();
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(capability),
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(BOOL_PRESENTATION_CONTRACT_REVISION),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(execution_profile),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(artifact),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![present_host_operation_requirement(
            kind_id(target),
            conduit_core::BOOL_ENCODED_LEN as u32,
        )],
        resource_requirements: vec![resource_requirement(PRESENTATION_RESOURCE_CLASS, 1)],
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_bool_presentation_catalog(
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    use conduit_form::{ConfigurationField, KindDefinition};
    let contract = bool_presentation_contract();
    profile
        .insert(KindDefinition {
            kind_id: contract.kind_id,
            kind_contract_revision: KindContractRevision::from(BOOL_PRESENTATION_CONTRACT_REVISION),
            inputs: contract.inputs,
            outputs: contract.outputs,
            configuration: Vec::<ConfigurationField>::new(),
        })
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bool_presenter_is_exact_current_boolean_to_admitted_effect() {
        let contract = bool_presentation_contract();
        for offer in [
            bool_presentation_browser_offer(),
            bool_presentation_std_offer(),
        ] {
            assert_eq!(contract.inputs[0].value_kind.as_str(), BOOL_INFO_ID);
            assert_eq!(contract.inputs[0].temporal, PortTemporal::Current);
            assert_eq!(contract.inputs, offer.inputs);
            assert_eq!(contract.limits, offer.limits);
            assert_eq!(offer.host_operations.len(), 1);
            assert_eq!(offer.resource_requirements.len(), 1);
        }
    }
}
