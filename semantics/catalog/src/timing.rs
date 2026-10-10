use super::{
    KindConfigurationField, KindConfigurationRule, KindTerminalBehavior, StandardKindContract,
};
#[cfg(feature = "plot-catalog")]
use alloc::string::String;
use alloc::string::ToString;
use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, AbnormalTerminalTransduction, CancellationTransduction, CapabilityLimits,
    CheckedValueContract, ConfigurationValue, FiniteTerminalEmission, FrontValueContract,
    FrontValueLocation, Kind, KindSemanticLaw, NormalCloseTransduction, PortDescriptor,
    PortDirection, PortTemporal, PreparedLeafSequenceEncoder, TerminalTransductionProfile,
    BOOL_INFO_ID, CANCELLATION_REQUEST_INFO_ID, EMPTY_INFO_ID, TERMINAL_INFO_ENCODED_LEN,
    TERMINAL_INFO_ID,
};

pub const TIME_DEBOUNCE_KIND: &str = "time/debounce";
pub const TIME_DEBOUNCE_CONTRACT_REVISION: &str = "conduit.std/time-debounce-bool@1";

pub const TIME_TIMEOUT_KIND: &str = "time/timeout";
pub const TIME_TIMEOUT_CONTRACT_REVISION: &str = "conduit.std/time-timeout-tick-bool@1";

pub const TIME_DELAY_KIND: &str = "time/delay";
pub const TIME_DELAY_CONTRACT_REVISION: &str = "conduit.std/time-delay-bool@1";

pub const TIME_THROTTLE_KIND: &str = "time/throttle";
pub const TIME_THROTTLE_CONTRACT_REVISION: &str = "conduit.std/time-throttle-bool-leading@1";

pub const TIME_DEADLINE_KIND: &str = "time/deadline";
pub const TIME_DEADLINE_CONTRACT_REVISION: &str = "conduit.std/time-deadline-cancellation@1";
pub const TIME_SAMPLE_KIND: &str = "time/sample";
pub const TIME_SAMPLE_CONTRACT_REVISION: &str = "conduit.std/time-sample@1";
pub const TIME_WINDOW_KIND: &str = "time/window";
pub const TIME_WINDOW_CONTRACT_REVISION: &str = "conduit.std/time-window@1";
pub const TIME_WINDOW_MAXIMUM_ITEMS: u16 = 8;

pub const TIME_POLICY_TRAILING: &str = "trailing";
pub const TIME_POLICY_LEADING: &str = "leading";
pub const TIME_MAXIMUM_DURATION_MS: u64 = 86_400_000;
pub const TIME_MAXIMUM_VALUES: u64 = 8;
pub const TIME_TIMEOUT_MAXIMUM_VALUES: u64 = 2;

pub fn time_debounce_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(TIME_DEBOUNCE_KIND),
        plain_name: "Trailing Boolean debounce".to_string(),
        summary: "Emit the last exact Boolean after one stable admitted duration; flush it on input closure."
            .to_string(),
        inputs: vec![port(
            "in",
            BOOL_INFO_ID,
            PortDirection::Input,
            PortTemporal::Current,
        )],
        outputs: vec![port(
            "out",
            BOOL_INFO_ID,
            PortDirection::Output,
            PortTemporal::Current,
        )],
        configuration: vec![
            duration_field(),
            KindConfigurationField {
                key: "policy".to_string(),
                default_value: ConfigurationValue::Text(TIME_POLICY_TRAILING.to_string()),
                rule: KindConfigurationRule::TextOneOf {
                    values: vec![TIME_POLICY_TRAILING.to_string()],
                },
            },
            maximum_values_field(TIME_MAXIMUM_VALUES),
        ],
        limits: limits(),
        terminal_behavior:
            KindTerminalBehavior::TrailingDebounceFlushesPendingValueThenCompletesWhenInputCloses,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "stable: time/debounce(duration-ms = 50, policy = \"trailing\")".to_string(),
    }
}

pub fn time_timeout_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(TIME_TIMEOUT_KIND),
        plain_name: "Tick inactivity timeout".to_string(),
        summary: "Emit false initially, true once after inactivity, and false again when exact tick activity recovers."
            .to_string(),
        inputs: vec![port(
            "activity",
            conduit_time::TICK_VALUE_KIND,
            PortDirection::Input,
            PortTemporal::Flow { closes: true },
        )],
        outputs: vec![port(
            "timed-out",
            BOOL_INFO_ID,
            PortDirection::Output,
            PortTemporal::Current,
        )],
        configuration: vec![
            duration_field(),
            maximum_values_field(TIME_TIMEOUT_MAXIMUM_VALUES),
        ],
        limits: limits(),
        terminal_behavior: KindTerminalBehavior::InactivityStateCancelsDeadlineAndCompletesWhenInputCloses,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "stale: time/timeout(duration-ms = 500)".to_string(),
    }
}

pub fn time_delay_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(TIME_DELAY_KIND),
        plain_name: "Ordered Boolean delay".to_string(),
        summary: "Emit every admitted Boolean in input order after one exact duration; drain admitted values on input closure.".to_string(),
        inputs: vec![port("in", BOOL_INFO_ID, PortDirection::Input, PortTemporal::Current)],
        outputs: vec![port("out", BOOL_INFO_ID, PortDirection::Output, PortTemporal::Current)],
        configuration: vec![duration_field(), maximum_values_field(TIME_MAXIMUM_VALUES)],
        limits: limits(),
        terminal_behavior: KindTerminalBehavior::DelaysEachValueInOrderAndDrainsOnInputClosure,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "paced: time/delay(duration-ms = 16ms, maximum-values = 8)".to_string(),
    }
}

pub fn time_throttle_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(TIME_THROTTLE_KIND),
        plain_name: "Leading Boolean throttle".to_string(),
        summary: "Emit the first admitted Boolean immediately, then drop values until the exact interval elapses.".to_string(),
        inputs: vec![port("in", BOOL_INFO_ID, PortDirection::Input, PortTemporal::Current)],
        outputs: vec![port("out", BOOL_INFO_ID, PortDirection::Output, PortTemporal::Current)],
        configuration: vec![
            duration_field(),
            KindConfigurationField {
                key: "policy".to_string(),
                default_value: ConfigurationValue::Text(TIME_POLICY_LEADING.to_string()),
                rule: KindConfigurationRule::TextOneOf {
                    values: vec![TIME_POLICY_LEADING.to_string()],
                },
            },
            maximum_values_field(TIME_MAXIMUM_VALUES),
        ],
        limits: limits(),
        terminal_behavior:
            KindTerminalBehavior::LeadingThrottleDropsValuesDuringIntervalAndCompletesWhenInputCloses,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "paced: time/throttle(duration-ms = 16ms, policy = \"leading\", maximum-values = 8)".to_string(),
    }
}

pub fn time_deadline_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(TIME_DEADLINE_KIND),
        plain_name: "One armed cancellation deadline".to_string(),
        summary: "After one Unit arm, emit one cancellation request when the exact admitted duration elapses; closing before the arm completes without output, while closing after the arm does not revoke the deadline. The output requests cancellation and does not claim that cancellation occurred."
            .to_string(),
        inputs: vec![port(
            "arm",
            EMPTY_INFO_ID,
            PortDirection::Input,
            PortTemporal::Flow { closes: true },
        )],
        outputs: vec![port(
            "request",
            CANCELLATION_REQUEST_INFO_ID,
            PortDirection::Output,
            PortTemporal::Value,
        )],
        configuration: vec![duration_field()],
        limits: limits(),
        terminal_behavior:
            KindTerminalBehavior::EmitsOneDecisionOrCompletesWhenDecisionBecomesImpossible,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "deadline: time/deadline(duration-ms = 2000)".to_string(),
    }
}

pub fn time_debounce_semantic_contract() -> Kind {
    semantic_contract(time_debounce_contract(), TIME_DEBOUNCE_CONTRACT_REVISION)
}

pub fn time_timeout_semantic_contract() -> Kind {
    semantic_contract(time_timeout_contract(), TIME_TIMEOUT_CONTRACT_REVISION)
}

pub fn time_delay_semantic_contract() -> Kind {
    semantic_contract(time_delay_contract(), TIME_DELAY_CONTRACT_REVISION)
}

pub fn time_throttle_semantic_contract() -> Kind {
    semantic_contract(time_throttle_contract(), TIME_THROTTLE_CONTRACT_REVISION)
}

pub fn time_deadline_semantic_contract() -> Kind {
    semantic_contract(time_deadline_contract(), TIME_DEADLINE_CONTRACT_REVISION)
}

/// Specializes `time/sample` to one exact already-checked finite value contract.
///
/// This is deliberately not authored generic syntax. Catalog assembly derives
/// the specialization from checked connected ports and installs this exact Fore.
pub fn time_sample_semantic_contract(value: &CheckedValueContract) -> Result<Kind, &'static str> {
    if value.maximum_bytes == 0 {
        return Err("time/sample requires one finite nonzero value envelope");
    }
    let value_port = |name: &str, direction| PortDescriptor {
        port_id: port_id(name),
        value_kind: value.value_kind.clone(),
        direction,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: None,
    };
    let inputs = vec![
        value_port("value", PortDirection::Input),
        port(
            "cadence",
            conduit_time::TICK_VALUE_KIND,
            PortDirection::Input,
            PortTemporal::Flow { closes: true },
        ),
    ];
    let outputs = vec![value_port("sample", PortDirection::Output)];
    let value_contracts = vec![
        FrontValueContract {
            location: FrontValueLocation::Input(port_id("value")),
            contract: value.clone(),
        },
        FrontValueContract {
            location: FrontValueLocation::Output(port_id("sample")),
            contract: value.clone(),
        },
    ];
    Ok(Kind {
        startup_parameters: Vec::new(),
        shorthand: None,
        kind_id: kind_id(TIME_SAMPLE_KIND),
        kind_contract_revision: TIME_SAMPLE_CONTRACT_REVISION.into(),
        inputs,
        outputs,
        configuration: Vec::new(),
        semantic_laws: vec![
            KindSemanticLaw::Terminal(
                KindTerminalBehavior::SamplesLatestValueAtCadenceAndCompletesWhenCadenceCloses,
            ),
            KindSemanticLaw::ValueContracts(value_contracts),
        ],
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 2,
            max_queue_bytes: value
                .maximum_bytes
                .checked_add(conduit_time::TICK_ENCODED_LEN)
                .ok_or("time/sample finite queue envelope overflows")?,
        },
    })
}

/// Specializes a processing-time tumbling window over one exact finite value
/// contract and one exact finite retained-item bound.
///
/// The duration remains authored configuration. The connected value contract
/// and reviewed implementation bound are non-authored specialization truth and
/// participate in the checked Fore and Plan identities.
pub fn time_window_semantic_contract(
    value: &CheckedValueContract,
    maximum_items: u16,
) -> Result<Kind, &'static str> {
    if value.maximum_bytes == 0 && value.value_kind.as_str() != EMPTY_INFO_ID {
        return Err("time/window requires one finite canonical value envelope");
    }
    let encoder = PreparedLeafSequenceEncoder::new(
        value.value_kind.clone(),
        value.maximum_bytes,
        maximum_items,
    )
    .map_err(|_| "time/window output exceeds structured Info bounds")?;
    let window = CheckedValueContract::new(
        encoder
            .value_type()
            .map_err(|_| "time/window output type is invalid")?
            .profile()
            .map_err(|_| "time/window output profile is invalid")?
            .value_kind()
            .clone(),
        encoder.maximum_bytes(),
        vec![],
    )
    .map_err(|_| "time/window output contract is invalid")?;
    let terminal = CheckedValueContract::new(
        kind_id(TERMINAL_INFO_ID),
        TERMINAL_INFO_ENCODED_LEN as u32,
        vec![],
    )
    .expect("canonical terminal info has one exact finite envelope");
    Ok(Kind {
        startup_parameters: super::startup_front(&[duration_field()]),
        shorthand: None,
        kind_id: kind_id(TIME_WINDOW_KIND),
        kind_contract_revision: TIME_WINDOW_CONTRACT_REVISION.into(),
        inputs: vec![PortDescriptor {
            port_id: port_id("value"),
            value_kind: value.value_kind.clone(),
            direction: PortDirection::Input,
            temporal: PortTemporal::Flow { closes: true },
            abnormal_kind: Some(kind_id(TERMINAL_INFO_ID)),
        }],
        outputs: vec![PortDescriptor {
            port_id: port_id("window"),
            value_kind: window.value_kind.clone(),
            direction: PortDirection::Output,
            temporal: PortTemporal::Flow { closes: true },
            abnormal_kind: Some(kind_id(TERMINAL_INFO_ID)),
        }],
        configuration: vec![duration_field()],
        semantic_laws: vec![
            KindSemanticLaw::Terminal(KindTerminalBehavior::TumblingProcessingTimeWindow {
                maximum_items,
            }),
            KindSemanticLaw::ValueContracts(vec![
                FrontValueContract {
                    location: FrontValueLocation::Input(port_id("value")),
                    contract: value.clone(),
                },
                FrontValueContract {
                    location: FrontValueLocation::Output(port_id("window")),
                    contract: window.clone(),
                },
                FrontValueContract {
                    location: FrontValueLocation::InputAbnormal(port_id("value")),
                    contract: terminal.clone(),
                },
                FrontValueContract {
                    location: FrontValueLocation::OutputAbnormal(port_id("window")),
                    contract: terminal,
                },
            ]),
            KindSemanticLaw::TerminalTransduction(TerminalTransductionProfile {
                input_port_id: port_id("value"),
                output_port_id: port_id("window"),
                normal_close: NormalCloseTransduction::FlushThenPropagate(FiniteTerminalEmission {
                    maximum_items: 1,
                    maximum_bytes: window.maximum_bytes,
                }),
                abnormal: AbnormalTerminalTransduction::DomainSpecific {
                    law: kind_id("time/window/abnormal-cancels-boundary@1"),
                },
                cancellation: CancellationTransduction::NotCancellable,
            }),
        ],
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: maximum_items
                .checked_add(2)
                .ok_or("time/window queue item envelope overflows")?,
            max_queue_bytes: value
                .maximum_bytes
                .checked_mul(u32::from(maximum_items))
                .and_then(|bytes| bytes.checked_add(window.maximum_bytes))
                .and_then(|bytes| bytes.checked_add(TERMINAL_INFO_ENCODED_LEN as u32))
                .ok_or("time/window finite queue envelope overflows")?,
        },
    })
}

#[cfg(feature = "plot-catalog")]
pub fn install_time_window_kind(
    value: &CheckedValueContract,
    maximum_items: u16,
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    startup.insert(conduit_plot::KindSignature {
        kind: TIME_WINDOW_KIND.to_string(),
        startup_parameters: vec![conduit_plot::StartupParameterSignature {
            name: "duration-ms".into(),
            value_type: "Duration".into(),
            default: Some("100ms".into()),
        }],
    })?;
    profile
        .insert_kind(time_window_semantic_contract(value, maximum_items).map_err(str::to_string)?)
        .map_err(|error| error.to_string())
}

#[cfg(feature = "plot-catalog")]
pub fn install_time_sample_kind(
    value: &CheckedValueContract,
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    startup.insert(conduit_plot::KindSignature {
        kind: TIME_SAMPLE_KIND.to_string(),
        startup_parameters: Vec::new(),
    })?;
    profile
        .insert_kind(time_sample_semantic_contract(value).map_err(str::to_string)?)
        .map_err(|error| error.to_string())
}

fn semantic_contract(contract: StandardKindContract, revision: &str) -> Kind {
    let startup_parameters = super::startup_front(&contract.configuration);
    Kind {
        startup_parameters,
        shorthand: None,
        kind_id: contract.kind_id,
        kind_contract_revision: revision.into(),
        inputs: contract.inputs,
        outputs: contract.outputs,
        configuration: contract.configuration,
        semantic_laws: alloc::vec![conduit_core::KindSemanticLaw::Terminal(
            contract.terminal_behavior
        )],
        limits: contract.limits,
    }
}

#[cfg(feature = "plot-catalog")]
pub fn install_timing_catalogs(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    use conduit_plot::{KindSignature, StartupParameterSignature};

    for contract in [
        time_debounce_contract(),
        time_timeout_contract(),
        time_delay_contract(),
        time_throttle_contract(),
        time_deadline_contract(),
    ] {
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().to_string(),
            startup_parameters: contract
                .configuration
                .iter()
                .map(|field| StartupParameterSignature {
                    name: field.key.clone(),
                    value_type: match field.key.as_str() {
                        "duration-ms" => "Duration",
                        "policy" => "Text",
                        _ => "Count",
                    }
                    .to_string(),
                    default: Some(configuration_source(field)),
                })
                .collect(),
        })?;
        let revision = match contract.kind_id.as_str() {
            TIME_DEBOUNCE_KIND => TIME_DEBOUNCE_CONTRACT_REVISION,
            TIME_TIMEOUT_KIND => TIME_TIMEOUT_CONTRACT_REVISION,
            TIME_DELAY_KIND => TIME_DELAY_CONTRACT_REVISION,
            TIME_THROTTLE_KIND => TIME_THROTTLE_CONTRACT_REVISION,
            TIME_DEADLINE_KIND => TIME_DEADLINE_CONTRACT_REVISION,
            _ => unreachable!("timing catalog loop contains only timing contracts"),
        };
        profile
            .insert_kind(semantic_contract(contract, revision))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn port(
    name: &str,
    info: &str,
    direction: PortDirection,
    temporal: PortTemporal,
) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(info),
        direction,
        temporal,
        abnormal_kind: None,
    }
}

fn duration_field() -> KindConfigurationField {
    KindConfigurationField {
        key: "duration-ms".to_string(),
        default_value: ConfigurationValue::Quantity(
            conduit_core::QuantityConfigurationValue::from_value(conduit_core::Quantity::new(
                100,
                conduit_core::Unit::Millisecond,
            ))
            .expect("bounded quantity configuration"),
        ),
        rule: KindConfigurationRule::QuantityRange {
            minimum: 0,
            maximum: TIME_MAXIMUM_DURATION_MS as i64,
            canonical_unit: conduit_core::Unit::Millisecond,
        },
    }
}

fn maximum_values_field(maximum: u64) -> KindConfigurationField {
    KindConfigurationField {
        key: "maximum-values".to_string(),
        default_value: ConfigurationValue::U64(maximum),
        rule: KindConfigurationRule::U64Range {
            minimum: 1,
            maximum,
        },
    }
}

fn limits() -> CapabilityLimits {
    CapabilityLimits {
        max_active_instances: 16,
        max_queue_items: 1,
        max_queue_bytes: 8,
    }
}

#[cfg(feature = "plot-catalog")]
fn configuration_source(field: &KindConfigurationField) -> alloc::string::String {
    match (&*field.key, &field.default_value) {
        (_, ConfigurationValue::Quantity(value)) => value.source().into(),
        (_, ConfigurationValue::Unit(value)) => value.source().into(),
        (_, ConfigurationValue::TemperatureDifference(value)) => value.source().into(),
        (_, ConfigurationValue::U64(value)) => value.to_string(),
        (_, ConfigurationValue::Text(value)) => alloc::format!("\"{value}\""),
        _ => unreachable!("timing contracts use only bounded Quantity, Count, and Text values"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_specialization_preserves_exact_value_identity_and_bound() {
        let text = CheckedValueContract::new(kind_id("value/text"), 73, Vec::new()).unwrap();
        let contract = time_sample_semantic_contract(&text).unwrap();
        assert_eq!(contract.inputs[0].value_kind, text.value_kind);
        assert_eq!(contract.outputs[0].value_kind, text.value_kind);
        assert_eq!(contract.limits.max_queue_items, 2);
        assert_eq!(
            contract.limits.max_queue_bytes,
            73 + conduit_time::TICK_ENCODED_LEN
        );
        let contracts = contract.value_contracts();
        assert_eq!(contracts.len(), 2);
        assert!(contracts.iter().all(|entry| entry.contract == text));
        contract.validate().unwrap();
    }

    #[test]
    fn sample_specializations_are_exact_and_unbounded_values_refuse() {
        let text = CheckedValueContract::new(kind_id("value/text"), 73, Vec::new()).unwrap();
        let record =
            CheckedValueContract::new(kind_id("structured-info/profile/sample@1"), 91, Vec::new())
                .unwrap();
        let text = time_sample_semantic_contract(&text).unwrap();
        let record = time_sample_semantic_contract(&record).unwrap();
        assert_ne!(text.checked_front(), record.checked_front());
        assert!(time_sample_semantic_contract(
            &CheckedValueContract::new(kind_id("value/empty"), 0, Vec::new()).unwrap()
        )
        .is_err());
    }

    #[test]
    fn window_specialization_is_one_exact_finite_processing_time_contract() {
        let text = CheckedValueContract::new(kind_id("value/text"), 32, Vec::new()).unwrap();
        let contract = time_window_semantic_contract(&text, 8).unwrap();
        assert_eq!(contract.inputs[0].value_kind, text.value_kind);
        assert_ne!(contract.outputs[0].value_kind, text.value_kind);
        assert_eq!(contract.terminal_transductions().count(), 1);
        assert!(matches!(
            contract.semantic_laws.iter().find_map(|law| match law {
                KindSemanticLaw::Terminal(behavior) => Some(behavior),
                _ => None,
            }),
            Some(KindTerminalBehavior::TumblingProcessingTimeWindow { maximum_items: 8 })
        ));
        let contracts = contract.value_contracts();
        assert_eq!(contracts.len(), 4);
        assert_eq!(
            contracts
                .iter()
                .find(|entry| entry.location == FrontValueLocation::Input(port_id("value")))
                .unwrap()
                .contract,
            text
        );
        contract.validate().unwrap();
    }

    #[test]
    fn authored_sample_plot_expands_with_the_non_authored_exact_specialization() {
        let text = CheckedValueContract::new(kind_id("value/text"), 73, Vec::new()).unwrap();
        let source = Kind {
            startup_parameters: Vec::new(),
            shorthand: None,
            kind_id: kind_id("test/text-flow-source"),
            kind_contract_revision: "test/text-flow-source@1".into(),
            inputs: Vec::new(),
            outputs: vec![PortDescriptor {
                port_id: port_id("out"),
                value_kind: text.value_kind.clone(),
                direction: PortDirection::Output,
                temporal: PortTemporal::Flow { closes: true },
                abnormal_kind: None,
            }],
            configuration: Vec::new(),
            semantic_laws: vec![KindSemanticLaw::ValueContracts(vec![FrontValueContract {
                location: FrontValueLocation::Output(port_id("out")),
                contract: text.clone(),
            }])],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: text.maximum_bytes,
            },
        };
        let sink = Kind {
            startup_parameters: Vec::new(),
            shorthand: None,
            kind_id: kind_id("test/text-flow-sink"),
            kind_contract_revision: "test/text-flow-sink@1".into(),
            inputs: vec![PortDescriptor {
                port_id: port_id("in"),
                value_kind: text.value_kind.clone(),
                direction: PortDirection::Input,
                temporal: PortTemporal::Flow { closes: true },
                abnormal_kind: None,
            }],
            outputs: Vec::new(),
            configuration: Vec::new(),
            semantic_laws: vec![KindSemanticLaw::ValueContracts(vec![FrontValueContract {
                location: FrontValueLocation::Input(port_id("in")),
                contract: text.clone(),
            }])],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: text.maximum_bytes,
            },
        };
        let mut startup = conduit_plot::StartupCatalog::new();
        let mut profile = conduit_plot::ProfileCatalog::new();
        conduit_time::install_tick_catalog(&mut startup, &mut profile).unwrap();
        for kind in [&source, &sink] {
            startup
                .insert(conduit_plot::KindSignature {
                    kind: kind.kind_id.as_str().to_string(),
                    startup_parameters: kind
                        .startup_parameters
                        .iter()
                        .map(|parameter| conduit_plot::StartupParameterSignature {
                            name: parameter.name.clone(),
                            value_type: parameter.value_type.as_str().to_string(),
                            default: None,
                        })
                        .collect(),
                })
                .unwrap();
            profile.insert_kind((*kind).clone()).unwrap();
        }
        install_time_sample_kind(&text, &mut startup, &mut profile).unwrap();
        let syntax = conduit_plot::parse_syntax_document(
            "plot sampled-text {\n values: test/text-flow-source\n cadence: time/tick(count = 2, period-ms = 1)\n sampler: time/sample\n sink: test/text-flow-sink\n values.out >> sampler.value\n cadence.tick >> sampler.cadence\n sampler.sample >> sink.in\n}.\n",
        );
        let checked = conduit_plot::check_syntax_document(&syntax, &startup).unwrap();
        let expanded =
            conduit_plot::expand_canonical_plot(&checked, "sampled-text", &profile).unwrap();
        let gear = expanded
            .gears
            .iter()
            .find(|gear| gear.kind_id.as_str() == TIME_SAMPLE_KIND)
            .unwrap();
        assert_eq!(gear.semantic_contract.value_contracts().len(), 2);
        assert!(gear
            .semantic_contract
            .value_contracts()
            .iter()
            .all(|entry| entry.contract == text));
    }

    #[test]
    fn timing_contracts_are_typed_bounded_and_share_one_exact_deadline_requirement() {
        for contract in [
            time_debounce_contract(),
            time_timeout_contract(),
            time_delay_contract(),
            time_throttle_contract(),
            time_deadline_contract(),
        ] {
            assert_eq!(contract.limits.max_queue_items, 1);
        }
        for contract in [
            time_debounce_contract(),
            time_timeout_contract(),
            time_delay_contract(),
            time_throttle_contract(),
            time_deadline_contract(),
        ] {
            assert!(matches!(
                contract.configuration[0].rule,
                KindConfigurationRule::QuantityRange {
                    minimum: 0,
                    maximum,
                    canonical_unit: conduit_core::Unit::Millisecond,
                } if maximum == TIME_MAXIMUM_DURATION_MS as i64
            ));
        }
    }
}
