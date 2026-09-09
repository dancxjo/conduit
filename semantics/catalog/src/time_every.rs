use super::{
    StandardConfigurationField, StandardConfigurationRule, StandardKindContract, TerminalBehavior,
};
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{kind_id, CapabilityLimits, ConfigurationValue};

pub fn time_every_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(conduit_time::TIME_EVERY_KIND),
        plain_name: "Recurring interval ticks".to_string(),
        summary:
            "Emit recurring typed ticks at one admitted duration interval until lifecycle termination."
                .to_string(),
        inputs: Vec::new(),
        outputs: conduit_time::time_every_outputs(),
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
            max_queue_items: 1,
            max_queue_bytes: conduit_time::TICK_ENCODED_LEN,
        },
        terminal_behavior: TerminalBehavior::HostObservationEndsOrFailsSource,
        hosted_implementation_required: true,
        browser_manifestation_honest: true,
        pico_manifestation_honest: false,
        example: "clock: time/every(1s)".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_has_one_required_duration_and_standing_open_flow() {
        let contract = time_every_contract();
        assert_eq!(contract.configuration.len(), 1);
        assert_eq!(contract.configuration[0].key, "freq");
        assert_eq!(
            contract.outputs[0].temporal,
            conduit_core::PortTemporal::Flow { closes: false }
        );
        assert_eq!(
            contract.terminal_behavior,
            TerminalBehavior::HostObservationEndsOrFailsSource
        );
        assert!(contract.browser_manifestation_honest);
        assert!(!contract.pico_manifestation_honest);
    }
}
