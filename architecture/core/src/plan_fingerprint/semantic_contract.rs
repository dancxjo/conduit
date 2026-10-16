//! Canonical encoding of one immutable Kind semantic contract.
use super::*;

pub(super) fn push_semantic_contract(
    canonical: &mut Vec<u8>,
    contract: &crate::KindSemanticContract,
) {
    use crate::{KindConfigurationRule as Rule, KindSemanticLaw as Law};

    push_string(canonical, "kind-semantic-contract@1");
    push_u32(canonical, contract.configuration.len() as u32);
    for field in &contract.configuration {
        push_string(canonical, &field.key);
        push_configuration_value(canonical, &field.default_value);
        match &field.rule {
            Rule::Any => canonical.push(0),
            Rule::Quantity => canonical.push(8),
            Rule::Unit => canonical.push(9),
            Rule::TemperatureDifference => canonical.push(10),
            Rule::U64Range { minimum, maximum } => {
                canonical.push(1);
                push_u64(canonical, *minimum);
                push_u64(canonical, *maximum);
            }
            Rule::I64Range { minimum, maximum } => {
                canonical.push(2);
                canonical.extend_from_slice(&minimum.to_le_bytes());
                canonical.extend_from_slice(&maximum.to_le_bytes());
            }
            Rule::DurationMillis { minimum, maximum } => {
                canonical.push(3);
                push_u64(canonical, *minimum);
                push_u64(canonical, *maximum);
            }
            Rule::QuantityRange {
                minimum,
                maximum,
                canonical_unit,
            } => {
                canonical.push(4);
                canonical.extend_from_slice(&minimum.to_le_bytes());
                canonical.extend_from_slice(&maximum.to_le_bytes());
                push_string(canonical, &canonical_unit.semantic_id());
            }
            Rule::DataLabel { maximum } => {
                canonical.push(11);
                push_u32(canonical, *maximum);
            }
            Rule::TextBytes { maximum } => {
                canonical.push(5);
                push_u32(canonical, *maximum);
            }
            Rule::TextOneOf { values } => {
                canonical.push(6);
                push_u32(canonical, values.len() as u32);
                for value in values {
                    push_string(canonical, value);
                }
            }
            Rule::Structured { profile } => {
                canonical.push(7);
                push_string(canonical, profile.as_str());
            }
        }
    }
    push_u32(canonical, contract.laws.len() as u32);
    for law in &contract.laws {
        match law {
            Law::ExactRealizationProperty(value) => {
                canonical.push(19);
                push_string(canonical, value.property_profile.as_str());
                push_string(canonical, &value.configuration_key);
            }
            Law::RealizationRequirement {
                property_profile,
                configuration_key,
            } => {
                canonical.push(18);
                push_string(canonical, property_profile.as_str());
                push_string(canonical, configuration_key);
            }
            Law::Terminal(value) => {
                use crate::KindTerminalBehavior as Terminal;
                canonical.push(0);
                match value {
                    Terminal::EmitsOnce => canonical.push(0),
                    Terminal::EmitsOnceWhenScopeIsEligible => canonical.push(1),
                    Terminal::CompletesAfterConfiguredCount => canonical.push(2),
                    Terminal::CompletesAfterFixedCount { count } => {
                        canonical.push(3);
                        push_u64(canonical, *count);
                    }
                    Terminal::CompletesWhenInputsClose => canonical.push(4),
                    Terminal::MirrorsInputTerminal => canonical.push(5),
                    Terminal::RetainsLatestUntilReleased => canonical.push(6),
                    Terminal::EmitsCurrentAndCompletesWhenInputCloses => canonical.push(7),
                    Terminal::CoupledAtomicFanoutAndMirrorsInputTerminal => canonical.push(8),
                    Terminal::FirstReadyLeftTieCancelsLoserOrCompletesWithoutWinner => {
                        canonical.push(24)
                    }
                    Terminal::CurrentBooleanGateDefaultsClosedAndCompletesWhenInputsClose => {
                        canonical.push(9)
                    }
                    Terminal::CurrentScalarSelectorCompletesWhenInputsClose => canonical.push(10),
                    Terminal::EmitsOneDecisionOrCompletesWhenDecisionBecomesImpossible => {
                        canonical.push(11)
                    }
                    Terminal::TrailingDebounceFlushesPendingValueThenCompletesWhenInputCloses => {
                        canonical.push(12)
                    }
                    Terminal::InactivityStateCancelsDeadlineAndCompletesWhenInputCloses => {
                        canonical.push(13)
                    }
                    Terminal::DelaysEachValueInOrderAndDrainsOnInputClosure => canonical.push(14),
                    Terminal::LeadingThrottleDropsValuesDuringIntervalAndCompletesWhenInputCloses => {
                        canonical.push(15)
                    }
                    Terminal::SamplesLatestValueAtCadenceAndCompletesWhenCadenceCloses => {
                        canonical.push(25)
                    }
                    Terminal::TumblingProcessingTimeWindow { maximum_items } => {
                        canonical.push(26);
                        canonical.extend_from_slice(&maximum_items.to_le_bytes());
                    }
                    Terminal::SimulatedCurrentObservationEmitsOnce => canonical.push(16),
                    Terminal::HostInputEndsOrFailsSource => canonical.push(17),
                    Terminal::HostObservationEndsOrFailsSource => canonical.push(18),
                    Terminal::EmitsInitialAndTogglesUntilInputCloses => canonical.push(19),
                    Terminal::EmitsOneField => canonical.push(20),
                    Terminal::EvolvesAfterTicksAndCompletesWhenTickCloses => canonical.push(21),
                    Terminal::PresentsEachFieldAndCompletesWhenInputCloses => canonical.push(22),
                    Terminal::CompletesAfterDockedRefusedOrDeadline => canonical.push(23),
                    Terminal::EmitsThroughSourceTerminalFlag => canonical.push(24),
                }
            }
            Law::TerminalTransduction(value) => {
                canonical.push(1);
                push_terminal_transduction(canonical, value);
            }
            Law::ExternalEffects(value) => {
                canonical.push(2);
                canonical.push(*value as u8);
            }
            Law::TemporalState(value) => {
                canonical.push(3);
                canonical.push(*value as u8);
            }
            Law::TimeDependence(value) => {
                canonical.push(4);
                canonical.push(*value as u8);
            }
            Law::RandomDependence(value) => {
                canonical.push(5);
                canonical.push(*value as u8);
            }
            Law::ResourceDependence(value) => {
                canonical.push(6);
                canonical.push(*value as u8);
            }
            Law::Suspension(value) => {
                canonical.push(7);
                canonical.push(*value as u8);
            }
            Law::Variability(value) => {
                canonical.push(8);
                canonical.push(*value as u8);
            }
            Law::Replay(value) => {
                use crate::ReplayBehavior as Replay;
                canonical.push(9);
                match value {
                    Replay::Exact => canonical.push(0),
                    Replay::Ineligible => canonical.push(1),
                    Replay::Idempotent { operation_key_kind } => {
                        canonical.push(2);
                        push_string(canonical, operation_key_kind.as_str());
                    }
                    Replay::Transactional {
                        transaction_contract,
                    } => {
                        canonical.push(3);
                        push_string(canonical, transaction_contract.as_str());
                    }
                    Replay::Compensatable {
                        compensation_contract,
                    } => {
                        canonical.push(4);
                        push_string(canonical, compensation_contract.as_str());
                    }
                }
            }
            Law::ResourcePorts(ports) => {
                canonical.push(10);
                push_u32(canonical, ports.len() as u32);
                for port in ports {
                    push_string(canonical, port.port_id.as_str());
                    push_string(canonical, port.class_id.as_str());
                    canonical.push(port.ownership as u8);
                    canonical.push(port.lifecycle as u8);
                    canonical.push(port.mobility as u8);
                }
            }
            Law::ValueContracts(contracts) => {
                canonical.push(11);
                push_u32(canonical, contracts.len() as u32);
                for value_contract in contracts {
                    match &value_contract.location {
                        crate::FrontValueLocation::Startup(name) => {
                            canonical.push(0);
                            push_string(canonical, name);
                        }
                        crate::FrontValueLocation::Input(port) => {
                            canonical.push(1);
                            push_string(canonical, port.as_str());
                        }
                        crate::FrontValueLocation::Output(port) => {
                            canonical.push(2);
                            push_string(canonical, port.as_str());
                        }
                        crate::FrontValueLocation::InputAbnormal(port) => {
                            canonical.push(3);
                            push_string(canonical, port.as_str());
                        }
                        crate::FrontValueLocation::OutputAbnormal(port) => {
                            canonical.push(4);
                            push_string(canonical, port.as_str());
                        }
                    }
                    push_value_contract(canonical, &value_contract.contract);
                }
            }
            Law::KeyedJoin(join) => {
                canonical.push(12);
                push_value_contract(canonical, &join.key);
                push_value_contract(canonical, &join.left_value);
                push_value_contract(canonical, &join.right_value);
                canonical.extend_from_slice(&join.maximum_pending_per_side.to_le_bytes());
                canonical.push(join.pairing as u8);
                canonical.push(join.output_order as u8);
                canonical.push(join.capacity as u8);
                canonical.push(join.unmatched_on_close as u8);
            }
            Law::BoundedCollect(collect) => {
                canonical.push(13);
                push_string(canonical, collect.input_port_id.as_str());
                push_string(canonical, collect.output_port_id.as_str());
                push_value_contract(canonical, &collect.element);
                push_value_contract(canonical, &collect.collection);
                canonical.extend_from_slice(&collect.maximum_items.to_le_bytes());
                push_value_contract(canonical, &collect.overflow_disposition);
            }
            Law::FlowSelect(select) => {
                canonical.push(14);
                push_string(canonical, select.input_port_id.as_str());
                push_string(canonical, select.output_port_id.as_str());
                push_string(canonical, select.predicate_input_kind.as_str());
                push_string(canonical, select.predicate_output_kind.as_str());
                canonical.extend_from_slice(&select.maximum_active.to_le_bytes());
                canonical.extend_from_slice(&select.maximum_queued.to_le_bytes());
                canonical.extend_from_slice(&select.maximum_items.to_le_bytes());
                canonical.push(select.invocation as u8);
                canonical.push(select.retained_input as u8);
                canonical.push(select.true_disposition as u8);
                canonical.push(select.false_disposition as u8);
            }
            Law::FlowFold(fold) => {
                canonical.push(15);
                push_string(canonical, fold.input_port_id.as_str());
                push_string(canonical, fold.output_port_id.as_str());
                push_value_contract(canonical, &fold.item);
                push_value_contract(canonical, &fold.accumulator);
                push_u32(canonical, fold.initial_accumulator.len() as u32);
                canonical.extend_from_slice(&fold.initial_accumulator);
                push_string(canonical, fold.combine_accumulator_port_id.as_str());
                push_string(canonical, fold.combine_item_port_id.as_str());
                push_string(canonical, fold.combine_output_port_id.as_str());
                canonical.extend_from_slice(&fold.maximum_active.to_le_bytes());
                canonical.extend_from_slice(&fold.maximum_queued.to_le_bytes());
                canonical.extend_from_slice(&fold.maximum_items.to_le_bytes());
                canonical.push(fold.invocation as u8);
                canonical.push(fold.close as u8);
                canonical.push(fold.abnormal as u8);
                canonical.push(fold.cancellation as u8);
            }
            Law::FlowScan(scan) => {
                canonical.push(17);
                push_string(canonical, scan.input_port_id.as_str());
                push_string(canonical, scan.output_port_id.as_str());
                push_value_contract(canonical, &scan.item);
                push_value_contract(canonical, &scan.accumulator);
                push_u32(canonical, scan.initial_accumulator.len() as u32);
                canonical.extend_from_slice(&scan.initial_accumulator);
                push_string(canonical, scan.combine_accumulator_port_id.as_str());
                push_string(canonical, scan.combine_item_port_id.as_str());
                push_string(canonical, scan.combine_output_port_id.as_str());
                canonical.extend_from_slice(&scan.maximum_active.to_le_bytes());
                canonical.extend_from_slice(&scan.maximum_queued.to_le_bytes());
                canonical.extend_from_slice(&scan.maximum_items.to_le_bytes());
                canonical.push(scan.invocation as u8);
                canonical.push(scan.progression as u8);
                canonical.push(scan.empty as u8);
                canonical.push(scan.close as u8);
                canonical.push(scan.abnormal as u8);
                canonical.push(scan.cancellation as u8);
            }
            Law::FlowEach(each) => {
                canonical.push(16);
                push_string(canonical, each.input_port_id.as_str());
                push_string(canonical, each.output_port_id.as_str());
                canonical.extend_from_slice(&each.maximum_items.to_le_bytes());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kind_id, ExactRealizationProperty, KindSemanticContract, KindSemanticLaw};

    fn encoded(law: KindSemanticLaw) -> Vec<u8> {
        let mut bytes = Vec::new();
        push_semantic_contract(
            &mut bytes,
            &KindSemanticContract {
                configuration: Vec::new(),
                laws: alloc::vec![law],
            },
        );
        bytes
    }

    #[test]
    fn exact_property_fingerprint_binds_match_mode_profile_and_key() {
        let request = ExactRealizationProperty {
            property_profile: kind_id("test/profile"),
            configuration_key: "signature".into(),
        };
        let original = encoded(KindSemanticLaw::ExactRealizationProperty(request.clone()));
        let mut other = request.clone();
        other.configuration_key = "other".into();
        assert_ne!(
            original,
            encoded(KindSemanticLaw::ExactRealizationProperty(other))
        );
        let mut other = request.clone();
        other.property_profile = kind_id("test/other-profile");
        assert_ne!(
            original,
            encoded(KindSemanticLaw::ExactRealizationProperty(other))
        );
        assert_ne!(
            original,
            encoded(KindSemanticLaw::RealizationRequirement {
                property_profile: request.property_profile,
                configuration_key: request.configuration_key
            })
        );
    }
}
