use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{
    ConfigurationValue, InfoBool, PlannedGear, PortDirection, PortTemporal, BOOL_ENCODED_LEN,
};
use conduit_kernel::{OperationAction, OperationInput, PortId, ValueRef, ValueStorage};

pub(super) static STATE_TOGGLE_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::STATE_TOGGLE_IMPLEMENTATION,
    budget: state_toggle_budget,
    prepare: prepare_state_toggle,
};

pub(super) struct StateToggleOperation {
    values: Vec<ValueRef>,
    next: usize,
    initial_emitted: bool,
}

impl StateToggleOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Emit {
            port: PortId(0),
            value: self.values[0],
        }
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.initial_emitted && value.byte_len == conduit_time::TICK_ENCODED_LEN => {
                self.next += 1;
                self.values.get(self.next).copied().map_or_else(
                    || InstalledOperation::fail(35),
                    |value| OperationAction::Emit {
                        port: PortId(0),
                        value,
                    },
                )
            }
            OperationInput::Closed { port: PortId(0) } if self.initial_emitted => {
                OperationAction::Complete
            }
            _ => InstalledOperation::fail(34),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        self.initial_emitted = true;
        OperationAction::Await
    }
}

fn state_toggle_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_state_toggle(placement)?;
    Ok(OperationBudget {
        value_items: conduit_std_catalog::MAX_TOGGLE_VALUES as u16,
        value_bytes: (BOOL_ENCODED_LEN as u64 * conduit_std_catalog::MAX_TOGGLE_VALUES) as u32,
        host_requests: 0,
        sign_items: 96,
        maximum_value_bytes: BOOL_ENCODED_LEN as u32,
    })
}

fn prepare_state_toggle(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_state_toggle(placement)?;
    let initial = placement
        .configuration
        .iter()
        .find_map(|entry| match (entry.key.as_str(), &entry.value) {
            ("initial", ConfigurationValue::Bool(value)) => Some(*value),
            _ => None,
        })
        .ok_or_else(|| "state/toggle configuration 'initial' is missing or invalid".to_string())?;
    let mut admitted = Vec::with_capacity(conduit_std_catalog::MAX_TOGGLE_VALUES as usize);
    for index in 0..conduit_std_catalog::MAX_TOGGLE_VALUES {
        let current = conduit_std_catalog::bounded_toggle_value(initial, index)
            .ok_or_else(|| "state/toggle exceeds its admitted value bound".to_string())?;
        admitted.push(
            values
                .store(&InfoBool::new(current).encode())
                .map_err(|error| format!("store toggle value {index}: {error:?}"))?,
        );
    }
    Ok(InstalledOperation::StateToggle(StateToggleOperation {
        values: admitted,
        next: 0,
        initial_emitted: false,
    }))
}

fn validate_state_toggle(placement: &PlannedGear) -> Result<(), String> {
    let input = placement.inputs.first();
    let output = placement.outputs.first();
    let configuration_exact = placement.configuration.len() == 1
        && placement.configuration[0].key == "initial"
        && matches!(
            placement.configuration[0].value,
            ConfigurationValue::Bool(_)
        );
    if placement.kind_id.as_str() != conduit_std_catalog::STATE_TOGGLE_KIND
        || placement.kind_contract_revision.as_str()
            != conduit_std_catalog::STATE_TOGGLE_CONTRACT_REVISION
        || placement.execution_profile_id.as_str()
            != conduit_std_offers::STATE_TOGGLE_EXECUTION_PROFILE
        || placement.implementation_id.as_str() != conduit_std_offers::STATE_TOGGLE_IMPLEMENTATION
        || placement.artifact_id.as_str() != conduit_std_offers::STATE_TOGGLE_ARTIFACT
        || placement.inputs.len() != 1
        || !input.is_some_and(|port| {
            port.port_id.as_str() == "toggle"
                && port.value_kind.as_str() == conduit_time::TICK_VALUE_KIND
                && port.direction == PortDirection::Input
                && port.temporal == PortTemporal::Flow { closes: true }
        })
        || placement.outputs.len() != 1
        || !output.is_some_and(|port| {
            port.port_id.as_str() == "value"
                && port.value_kind.as_str() == conduit_core::BOOL_INFO_ID
                && port.direction == PortDirection::Output
                && port.temporal == PortTemporal::Current
        })
        || !configuration_exact
    {
        return Err("planned state/toggle identity does not match its installation".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::{Failure, FailureCode};

    fn value(slot: u16, byte_len: u32) -> ValueRef {
        ValueRef {
            slot,
            generation: 1,
            byte_len,
        }
    }

    fn operation(initial: bool) -> StateToggleOperation {
        let levels = if initial {
            [11, 10, 11, 10]
        } else {
            [10, 11, 10, 11]
        };
        StateToggleOperation {
            values: levels
                .into_iter()
                .map(|slot| value(slot, BOOL_ENCODED_LEN as u32))
                .collect(),
            next: 0,
            initial_emitted: false,
        }
    }

    #[test]
    fn emits_exact_initial_value_then_alternates_until_input_closes() {
        let mut toggle = operation(true);
        assert_eq!(
            toggle.start(),
            OperationAction::Emit {
                port: PortId(0),
                value: value(11, BOOL_ENCODED_LEN as u32),
            }
        );
        assert_eq!(toggle.advance(), OperationAction::Await);

        for expected in [10, 11, 10] {
            assert_eq!(
                toggle.resume(OperationInput::Value {
                    port: PortId(0),
                    value: value(20, conduit_time::TICK_ENCODED_LEN),
                }),
                OperationAction::Emit {
                    port: PortId(0),
                    value: value(expected, BOOL_ENCODED_LEN as u32),
                }
            );
            assert_eq!(toggle.advance(), OperationAction::Await);
        }
        assert_eq!(
            toggle.resume(OperationInput::Closed { port: PortId(0) }),
            OperationAction::Complete
        );
    }

    #[test]
    fn refuses_input_before_initial_delivery_and_malformed_tick_identity() {
        for input in [
            OperationInput::Value {
                port: PortId(0),
                value: value(20, conduit_time::TICK_ENCODED_LEN),
            },
            OperationInput::Closed { port: PortId(0) },
        ] {
            assert_eq!(
                operation(false).resume(input),
                OperationAction::Fail(Failure {
                    code: FailureCode::InvalidLifecycle,
                    detail: 34,
                })
            );
        }

        let mut toggle = operation(false);
        assert!(matches!(toggle.start(), OperationAction::Emit { .. }));
        assert_eq!(toggle.advance(), OperationAction::Await);
        for input in [
            OperationInput::Value {
                port: PortId(1),
                value: value(20, conduit_time::TICK_ENCODED_LEN),
            },
            OperationInput::Value {
                port: PortId(0),
                value: value(20, conduit_time::TICK_ENCODED_LEN - 1),
            },
        ] {
            assert_eq!(
                toggle.resume(input),
                OperationAction::Fail(Failure {
                    code: FailureCode::InvalidLifecycle,
                    detail: 34,
                })
            );
        }
    }
}
