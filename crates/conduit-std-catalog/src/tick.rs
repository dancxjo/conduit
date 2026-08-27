use super::{
    StandardConfigurationField, StandardConfigurationRule, StandardKindContract, TerminalBehavior,
};
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{CapabilityLimits, ConfigurationValue};

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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn contract_uses_portable_time_identity() {
        let contract = tick_contract();
        assert_eq!(contract.kind_id.as_str(), conduit_time::TICK_KIND);
        assert_eq!(contract.outputs, conduit_time::tick_outputs());
    }
}
