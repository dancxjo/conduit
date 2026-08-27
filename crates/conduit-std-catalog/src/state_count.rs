use super::{
    StandardConfigurationField, StandardConfigurationRule, StandardKindContract, TerminalBehavior,
};
#[cfg(feature = "form-catalog")]
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
#[cfg(feature = "form-catalog")]
use conduit_core::KindContractRevision;
use conduit_core::{
    kind_id, port_id, CapabilityLimits, ConfigurationValue, PortDescriptor, PortDirection,
    PortTemporal,
};

pub const STATE_COUNT_KIND: &str = "state/count";
pub const STATE_COUNT_VALUE_KIND: &str = "value/count@1";
pub const STATE_COUNT_CONTRACT_REVISION: &str = "conduit.std/state-count@1";

pub const COUNT_PRESENTATION_KIND: &str = "presentation/count";
pub const COUNT_PRESENTATION_CONTRACT_REVISION: &str = "conduit.std/presentation-count@1";
pub const COUNT_ENCODED_LEN: u32 = 8;
pub const MAX_COUNT_VALUES: u64 = conduit_time::TIME_EVERY_COUNT + 1;

pub const fn bounded_count_value(start: u64, index: u64) -> Option<u64> {
    if index < MAX_COUNT_VALUES {
        start.checked_add(index)
    } else {
        None
    }
}

pub fn state_count_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(STATE_COUNT_KIND),
        plain_name: "Current count".to_string(),
        summary: "Emit an initial count and one current count after each closing-flow tick."
            .to_string(),
        inputs: vec![PortDescriptor {
            port_id: port_id("bump"),
            value_kind: kind_id(conduit_time::TICK_VALUE_KIND),
            direction: PortDirection::Input,
            temporal: PortTemporal::Flow { closes: true },
        }],
        outputs: vec![PortDescriptor {
            port_id: port_id("value"),
            value_kind: kind_id(STATE_COUNT_VALUE_KIND),
            direction: PortDirection::Output,
            temporal: PortTemporal::Current,
        }],
        configuration: vec![StandardConfigurationField {
            key: "start".to_string(),
            default_value: ConfigurationValue::U64(0),
            rule: StandardConfigurationRule::U64Range {
                minimum: 0,
                maximum: u64::MAX - conduit_time::TIME_EVERY_COUNT,
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
        example: "count: state/count(0)".to_string(),
    }
}

pub fn count_presentation_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(COUNT_PRESENTATION_KIND),
        plain_name: "Count presentation".to_string(),
        summary: "Present up to five exact current count observations on stdout.".to_string(),
        inputs: vec![PortDescriptor {
            port_id: port_id("value"),
            value_kind: kind_id(STATE_COUNT_VALUE_KIND),
            direction: PortDirection::Input,
            temporal: PortTemporal::Current,
        }],
        outputs: Vec::new(),
        configuration: vec![StandardConfigurationField {
            key: "maximum-values".to_string(),
            default_value: ConfigurationValue::U64(MAX_COUNT_VALUES),
            rule: StandardConfigurationRule::U64Range {
                minimum: 1,
                maximum: MAX_COUNT_VALUES,
            },
        }],
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: MAX_COUNT_VALUES as u16,
            max_queue_bytes: 64,
        },
        terminal_behavior: TerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "show: presentation/count".to_string(),
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_count_pipeline_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    use conduit_form::{ConfigurationField, ConfigurationRule, KindDefinition, KindSignature};
    for (contract, revision) in [
        (state_count_contract(), STATE_COUNT_CONTRACT_REVISION),
        (
            count_presentation_contract(),
            COUNT_PRESENTATION_CONTRACT_REVISION,
        ),
    ] {
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().to_string(),
            startup_parameters: contract
                .configuration
                .iter()
                .map(|field| conduit_form::StartupParameterSignature {
                    name: field.key.clone(),
                    value_type: "Count".to_string(),
                    default: Some(match field.default_value {
                        ConfigurationValue::U64(value) => value.to_string(),
                        _ => unreachable!("count family only has Count startup values"),
                    }),
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
                            StandardConfigurationRule::U64Range { minimum, maximum } => {
                                ConfigurationRule::U64Range { minimum, maximum }
                            }
                            _ => unreachable!("count family only has Count ranges"),
                        },
                    })
                    .collect(),
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_family_distinguishes_closing_ticks_from_current_counts() {
        let state = state_count_contract();
        assert_eq!(
            state.inputs[0].temporal,
            PortTemporal::Flow { closes: true }
        );
        assert_eq!(state.outputs[0].temporal, PortTemporal::Current);
        assert_eq!(state.configuration[0].key, "start");

        let presentation = count_presentation_contract();
        assert_eq!(presentation.inputs[0].temporal, PortTemporal::Current);
        assert_ne!(state.kind_id, presentation.kind_id);
    }

    #[cfg(feature = "form-catalog")]
    #[test]
    fn count_family_installs_exact_source_contracts() {
        let mut startup = conduit_form::StartupCatalog::new();
        let mut profile = conduit_form::ProfileCatalog::new();
        conduit_time::install_time_every_catalog(&mut startup, &mut profile).unwrap();
        crate::install_tick_presentation_catalog(&mut startup, &mut profile).unwrap();
        install_count_pipeline_catalogs(&mut startup, &mut profile).unwrap();
        let source = "form count (\n    start: Count = 0\n    bump: Tick...| > value: $Count\n) {\n    gear: state/count(start)\n    bump > gear.bump\n    gear.value > value\n}\nform main {\n    clock: time/every(1s)\n    count: count\n    show: presentation/count\n    clock > count > show\n}\n";
        let syntax = conduit_form::parse_syntax_document(source);
        let checked = conduit_form::check_syntax_document(&syntax, &startup).unwrap();
        let expanded = conduit_form::expand_canonical_form(&checked, "main", &profile).unwrap();
        let count = checked
            .forms
            .iter()
            .find(|form| form.name == "count")
            .unwrap();
        let state = expanded
            .gears
            .iter()
            .find(|operation| operation.kind_id.as_str() == STATE_COUNT_KIND)
            .unwrap();
        assert_eq!(count.checked_face(), state.checked_face());
    }
}
