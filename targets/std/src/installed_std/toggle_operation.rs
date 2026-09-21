use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{
    ConfigurationValue, InfoBool, PlannedGear, PortDirection, PortTemporal, BOOL_ENCODED_LEN,
};
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    Failure, FailureCode, PortId, ValueRef, ValueStorage,
};

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

impl<const PORTS: usize> StepOperation<PORTS> for StateToggleOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if !self.initial_emitted {
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            let Some(value) = self.values.first().copied() else {
                return StepOutcome::Fail(toggle_failure(35));
            };
            io.send(PortId(0), value)
                .expect("ready initial toggle output");
            self.initial_emitted = true;
            return StepOutcome::Progress;
        }
        if let Some(tick) = io.input(PortId(0)) {
            if tick.byte_len != conduit_time::TICK_ENCODED_LEN {
                return StepOutcome::Fail(toggle_failure(34));
            }
            let Some(next) = self.next.checked_add(1) else {
                return StepOutcome::Fail(toggle_failure(35));
            };
            let Some(value) = self.values.get(next).copied() else {
                return StepOutcome::Fail(toggle_failure(35));
            };
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            io.consume(PortId(0)).expect("present toggle tick");
            io.send(PortId(0), value).expect("ready toggle output");
            self.next = next;
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) {
            io.consume_closed(PortId(0))
                .expect("observed toggle input closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
}

fn toggle_failure(detail: u16) -> Failure {
    Failure {
        code: FailureCode::InvalidLifecycle,
        detail,
    }
}

impl StateToggleOperation {}

fn state_toggle_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_state_toggle(placement)?;
    Ok(OperationBudget {
        value_items: conduit_semantic_catalog::MAX_TOGGLE_VALUES as u16,
        value_bytes: (BOOL_ENCODED_LEN as u64 * conduit_semantic_catalog::MAX_TOGGLE_VALUES) as u32,
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
    let mut admitted = Vec::with_capacity(conduit_semantic_catalog::MAX_TOGGLE_VALUES as usize);
    for index in 0..conduit_semantic_catalog::MAX_TOGGLE_VALUES {
        let current = conduit_semantic_catalog::bounded_toggle_value(initial, index)
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
    if placement.kind_id.as_str() != conduit_semantic_catalog::STATE_TOGGLE_KIND
        || placement.kind_contract_revision.as_str()
            != conduit_semantic_catalog::STATE_TOGGLE_CONTRACT_REVISION
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
    use conduit_kernel::scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome};

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
        let mut io = StepIo::test_frame([None], [false], [Some(BOOL_ENCODED_LEN as u32)], None, 8);
        assert_eq!(
            toggle.step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Progress
        );
        assert_eq!(
            io.test_output(PortId(0)),
            Some(value(11, BOOL_ENCODED_LEN as u32))
        );

        for expected in [10, 11, 10] {
            let tick = value(20, conduit_time::TICK_ENCODED_LEN);
            let encoded = conduit_time::encode_tick(1);
            let mut io = StepIo::test_frame(
                [Some(tick)],
                [false],
                [Some(BOOL_ENCODED_LEN as u32)],
                None,
                8,
            );
            assert_eq!(
                toggle.step(&mut io, &StepInputBytes::test_frame([Some(&encoded)], None)),
                StepOutcome::Progress
            );
            assert!(io.test_consumed(PortId(0)));
            assert_eq!(
                io.test_output(PortId(0)),
                Some(value(expected, BOOL_ENCODED_LEN as u32))
            );
        }
        let mut io = StepIo::test_frame([None], [true], [None], None, 8);
        assert_eq!(
            toggle.step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Complete
        );
        assert!(io.test_consumed_closed(PortId(0)));
    }

    #[test]
    fn stages_initial_delivery_before_input_and_refuses_malformed_ticks() {
        let mut toggle = operation(false);
        let tick = value(20, conduit_time::TICK_ENCODED_LEN);
        let encoded = conduit_time::encode_tick(1);
        let mut io = StepIo::test_frame(
            [Some(tick)],
            [false],
            [Some(BOOL_ENCODED_LEN as u32)],
            None,
            8,
        );
        assert_eq!(
            toggle.step(&mut io, &StepInputBytes::test_frame([Some(&encoded)], None)),
            StepOutcome::Progress
        );
        assert!(!io.test_consumed(PortId(0)));
        assert_eq!(
            io.test_output(PortId(0)),
            Some(value(10, BOOL_ENCODED_LEN as u32))
        );

        let malformed = value(20, conduit_time::TICK_ENCODED_LEN - 1);
        let mut io = StepIo::test_frame([Some(malformed)], [false], [Some(1)], None, 8);
        assert_eq!(
            toggle.step(
                &mut io,
                &StepInputBytes::test_frame([Some(&encoded[..7])], None)
            ),
            StepOutcome::Fail(toggle_failure(34))
        );
    }
}
