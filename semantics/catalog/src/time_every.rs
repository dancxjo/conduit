use super::{
    KindConfigurationField, KindConfigurationRule, KindTerminalBehavior, StandardKindContract,
};
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{kind_id, CapabilityLimits, ConfigurationValue, Kind, Quantity, Unit};

pub fn time_every_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(conduit_time::TIME_EVERY_KIND),
        plain_name: "Recurring interval ticks".to_string(),
        summary:
            "Emit recurring typed ticks at one admitted duration interval until lifecycle termination."
                .to_string(),
        inputs: Vec::new(),
        outputs: conduit_time::time_every_outputs(),
        configuration: vec![KindConfigurationField {
            key: "freq".to_string(),
            default_value: ConfigurationValue::Quantity(conduit_core::QuantityConfigurationValue::from_value(Quantity::new(1_000, Unit::Millisecond)).expect("bounded quantity configuration")),
            rule: KindConfigurationRule::QuantityRange {
                minimum: 0,
                maximum: i64::MAX,
                canonical_unit: Unit::Millisecond.into(),
            },
        }],
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 4,
            max_queue_bytes: 64,
        },
        terminal_behavior: KindTerminalBehavior::HostObservationEndsOrFailsSource,
        hosted_implementation_required: true,
        browser_manifestation_honest: true,
        pico_manifestation_honest: false,
        example: "clock: time/every(1s)".to_string(),
    }
}

pub fn time_every_semantic_contract() -> Kind {
    conduit_time::time_every_semantic_contract()
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
            KindTerminalBehavior::HostObservationEndsOrFailsSource
        );
        assert!(contract.browser_manifestation_honest);
        assert!(!contract.pico_manifestation_honest);
        assert_eq!(contract.limits.max_queue_items, 4);
        assert_eq!(contract.limits.max_queue_bytes, 64);
        let semantics = time_every_semantic_contract();
        assert_eq!(
            semantics.startup_parameters[0].value_type.as_str(),
            conduit_core::DURATION_INFO_ID
        );
        assert!(!semantics.startup_parameters[0].has_default);
    }
}
