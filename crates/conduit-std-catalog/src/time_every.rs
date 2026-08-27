use super::{
    StandardConfigurationField, StandardConfigurationRule, StandardKindContract, TerminalBehavior,
};
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    kind_id, resource_requirement, wait_host_operation_requirement, ArtifactId, CapabilityId,
    CapabilityLimits, CapabilityOffer, ConfigurationValue, ExecutionProfileId, ImplementationId,
    KindContractRevision, TIMER_RESOURCE_CLASS,
};

pub const TIME_EVERY_EXECUTION_PROFILE: &str = "conduit.std/time-every-kernel-hosted@1";
pub const TIME_EVERY_IMPLEMENTATION: &str = "std/kernel-time-every@1";
pub const TIME_EVERY_ARTIFACT: &str = "conduit-std-host/time-every@1";
pub const TIME_EVERY_CAPABILITY: &str = "time-every-v1";

pub fn time_every_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(conduit_time::TIME_EVERY_KIND),
        plain_name: "Bounded interval ticks".to_string(),
        summary: "Emit exactly four typed ticks at one admitted duration interval.".to_string(),
        inputs: Vec::new(),
        outputs: conduit_time::tick_outputs(),
        configuration: vec![StandardConfigurationField {
            key: "freq".to_string(),
            default_value: ConfigurationValue::U64(1_000),
            rule: StandardConfigurationRule::DurationMillis {
                minimum: 0,
                maximum: u64::MAX,
            },
        }],
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: conduit_time::TIME_EVERY_COUNT as u16,
            max_queue_bytes: 64,
        },
        terminal_behavior: TerminalBehavior::CompletesAfterFixedCount {
            count: conduit_time::TIME_EVERY_COUNT,
        },
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "clock: time/every(1s)".to_string(),
    }
}

pub fn time_every_offer() -> CapabilityOffer {
    let contract = time_every_contract();
    CapabilityOffer {
        startup_parameters: vec![conduit_core::FaceStartupParameter {
            name: "freq".to_string(),
            value_type: "Duration".to_string(),
            has_default: false,
        }],
        shorthand: None,
        capability_id: CapabilityId::from(TIME_EVERY_CAPABILITY),
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(
            conduit_time::TIME_EVERY_CONTRACT_REVISION,
        ),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(TIME_EVERY_EXECUTION_PROFILE),
            implementation_id: ImplementationId::from(TIME_EVERY_IMPLEMENTATION),
            artifact_id: ArtifactId::from(TIME_EVERY_ARTIFACT),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![wait_host_operation_requirement()],
        resource_requirements: vec![resource_requirement(TIMER_RESOURCE_CLASS, 1)],
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_has_one_required_duration_and_fixed_finite_terminal() {
        let contract = time_every_contract();
        let offer = time_every_offer();
        assert_eq!(offer.startup_parameters.len(), 1);
        assert_eq!(offer.startup_parameters[0].name, "freq");
        assert!(!offer.startup_parameters[0].has_default);
        assert_eq!(
            contract.terminal_behavior,
            TerminalBehavior::CompletesAfterFixedCount {
                count: conduit_time::TIME_EVERY_COUNT
            }
        );
        assert!(!contract.browser_manifestation_honest && !contract.pico_manifestation_honest);
    }
}
