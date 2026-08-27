use super::{
    StandardConfigurationField, StandardConfigurationRule, StandardKindContract, TerminalBehavior,
};
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    resource_requirement, wait_host_operation_requirement, ArtifactId, CapabilityId,
    CapabilityLimits, CapabilityOffer, ConfigurationValue, ExecutionProfileId,
    HostOperationRequirement, ImplementationId, KindContractRevision, ResourceRequirement,
    TIMER_RESOURCE_CLASS,
};

pub const TICK_EXECUTION_PROFILE: &str = "conduit.std/time-tick-kernel-hosted@2";
pub const TICK_IMPLEMENTATION: &str = "std/kernel-time-tick@2";
pub const TICK_ARTIFACT: &str = "conduit-std-host/time-tick@2";
pub const TICK_CAPABILITY: &str = "time-tick-v2";

pub fn tick_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: conduit_core::kind_id(conduit_time::TICK_KIND),
        plain_name: "Tick".to_string(),
        summary: "Emit a finite sequence of typed timer ticks.".to_string(),
        inputs: Vec::new(),
        outputs: conduit_time::tick_outputs(),
        configuration: vec![
            StandardConfigurationField {
                key: "count".to_string(),
                default_value: ConfigurationValue::U64(4),
                rule: StandardConfigurationRule::U64Range {
                    minimum: 0,
                    maximum: conduit_time::MAX_TICK_COUNT,
                },
            },
            StandardConfigurationField {
                key: "period-ms".to_string(),
                default_value: ConfigurationValue::U64(1_000),
                rule: StandardConfigurationRule::U64Range {
                    minimum: 0,
                    maximum: u64::MAX,
                },
            },
        ],
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 4,
            max_queue_bytes: 64,
        },
        terminal_behavior: TerminalBehavior::CompletesAfterConfiguredCount,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "clock: time/tick".to_string(),
    }
}

pub fn tick_contract_revision() -> KindContractRevision {
    KindContractRevision::from(conduit_time::TICK_CONTRACT_REVISION)
}
pub fn tick_execution_profile() -> ExecutionProfileId {
    ExecutionProfileId::from(TICK_EXECUTION_PROFILE)
}
pub fn tick_host_operation_requirements() -> Vec<HostOperationRequirement> {
    vec![wait_host_operation_requirement()]
}
pub fn tick_resource_requirements() -> Vec<ResourceRequirement> {
    vec![resource_requirement(TIMER_RESOURCE_CLASS, 1)]
}

pub fn tick_capability_offer() -> CapabilityOffer {
    let contract = tick_contract();
    CapabilityOffer {
        startup_parameters: tick_face_startup_parameters(),
        shorthand: None,
        capability_id: CapabilityId::from(TICK_CAPABILITY),
        kind_id: contract.kind_id,
        kind_contract_revision: tick_contract_revision(),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: tick_execution_profile(),
            implementation_id: ImplementationId::from(TICK_IMPLEMENTATION),
            artifact_id: ArtifactId::from(TICK_ARTIFACT),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: tick_host_operation_requirements(),
        resource_requirements: tick_resource_requirements(),
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

pub fn tick_face_startup_parameters() -> Vec<conduit_core::FaceStartupParameter> {
    vec![
        conduit_core::FaceStartupParameter {
            name: "count".to_string(),
            value_type: "Count".to_string(),
            has_default: true,
        },
        conduit_core::FaceStartupParameter {
            name: "period-ms".to_string(),
            value_type: "Count".to_string(),
            has_default: true,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hosted_offer_uses_portable_contract() {
        let contract = tick_contract();
        assert_eq!(contract.kind_id.as_str(), conduit_time::TICK_KIND);
        assert_eq!(contract.outputs, conduit_time::tick_outputs());
        assert_eq!(
            tick_contract_revision().as_str(),
            conduit_time::TICK_CONTRACT_REVISION
        );
    }
}
