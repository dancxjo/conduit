//! Deterministic bounded first-value arbitration.

use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::{
    CheckedValueContract, PlannedGear, PortDirection, CANCELLATION_REQUEST_INFO_ID,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    PortId, ValueRef, ValueStorage,
};

pub(super) static FLOW_FIRST_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::FLOW_FIRST_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct FlowFirstBack {
    cancellation: Option<ValueRef>,
    left_closed: bool,
    right_closed: bool,
    decided: bool,
}

impl<const PORTS: usize> StepBack<PORTS> for FlowFirstBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.decided {
            return StepOutcome::Complete;
        }
        let left = io.input(PortId(0));
        let right = io.input(PortId(1));
        if let Some(winner) = left.or(right) {
            let left_wins = left.is_some();
            let cancellation_port = PortId(if left_wins { 2 } else { 1 });
            if !io.output_ready(PortId(0)) || !io.output_ready(cancellation_port) {
                return StepOutcome::Await;
            }
            let winner_port = PortId(if left_wins { 0 } else { 1 });
            io.consume(winner_port).expect("present flow/first winner");
            if left.is_some() && right.is_some() {
                let loser = io
                    .take_input(PortId(1))
                    .expect("simultaneous right loser remains present");
                io.discard(loser)
                    .expect("discard simultaneous losing value");
            }
            io.send(PortId(0), winner)
                .expect("ready flow/first winner output");
            io.send(
                cancellation_port,
                self.cancellation
                    .take()
                    .expect("one prepared loser cancellation"),
            )
            .expect("ready flow/first cancellation output");
            self.decided = true;
            return StepOutcome::Progress;
        }
        let mut progressed = false;
        for (port, closed) in [
            (PortId(0), &mut self.left_closed),
            (PortId(1), &mut self.right_closed),
        ] {
            if !*closed && io.input_closed(port) {
                io.consume_closed(port)
                    .expect("flow/first observes exact normal closure");
                *closed = true;
                progressed = true;
            }
        }
        if self.left_closed && self.right_closed {
            StepOutcome::Complete
        } else if progressed {
            StepOutcome::Progress
        } else {
            StepOutcome::Await
        }
    }

    fn cancel(&mut self) {
        self.cancellation = None;
        self.decided = true;
    }
}

fn exact_value_contract(placement: &PlannedGear) -> Result<&CheckedValueContract, String> {
    let contracts = placement.semantic_contract.value_contracts();
    if contracts.is_empty() {
        return Err("flow/first placement has no exact value contracts".into());
    }
    let winner = contracts
        .iter()
        .find(|entry| {
            entry.location
                == conduit_core::FrontValueLocation::Output(conduit_core::port_id(
                    conduit_semantic_catalog::OUT_PORT,
                ))
        })
        .ok_or("flow/first placement has no winner value contract")?;
    Ok(&winner.contract)
}

fn validate(placement: &PlannedGear) -> Result<&CheckedValueContract, String> {
    let value = exact_value_contract(placement)?;
    let expected = conduit_semantic_catalog::flow_first_contract(value).map_err(str::to_string)?;
    if placement.kind_id.as_str() != conduit_semantic_catalog::FIRST_KIND
        || placement.kind_contract_revision.as_str()
            != conduit_semantic_catalog::FLOW_FIRST_CONTRACT_REVISION
        || placement.execution_profile_id.as_str()
            != conduit_std_offers::FLOW_FIRST_EXECUTION_PROFILE
        || placement.implementation_id.as_str() != conduit_std_offers::FLOW_FIRST_IMPLEMENTATION
        || placement.artifact_id.as_str() != conduit_std_offers::FLOW_FIRST_ARTIFACT
        || placement.inputs != expected.inputs
        || placement.outputs != expected.outputs
        || placement.semantic_contract != expected.semantic_contract()
        || placement
            .inputs
            .iter()
            .any(|port| port.direction != PortDirection::Input)
        || placement
            .outputs
            .iter()
            .any(|port| port.direction != PortDirection::Output)
        || placement.outputs[1].value_kind.as_str() != CANCELLATION_REQUEST_INFO_ID
        || placement.outputs[2].value_kind.as_str() != CANCELLATION_REQUEST_INFO_ID
    {
        return Err("planned flow/first identity differs from its exact specialization".into());
    }
    Ok(value)
}

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    let maximum = validate(placement)?.maximum_bytes;
    Ok(BackBudget {
        value_items: 1,
        value_bytes: 0,
        host_requests: 0,
        sign_items: 64,
        maximum_value_bytes: maximum,
    })
}

fn prepare(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    validate(placement)?;
    let cancellation = values
        .store(&[])
        .map_err(|error| format!("prepare flow/first cancellation: {error:?}"))?;
    Ok(InstalledBack::FlowFirst(FlowFirstBack {
        cancellation: Some(cancellation),
        left_closed: false,
        right_closed: false,
        decided: false,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(slot: u16) -> ValueRef {
        ValueRef {
            slot,
            generation: 1,
            byte_len: conduit_core::SCALAR_ENCODED_LEN as u32,
        }
    }

    fn cancellation(slot: u16) -> ValueRef {
        ValueRef {
            slot,
            generation: 1,
            byte_len: 0,
        }
    }

    fn operation() -> FlowFirstBack {
        FlowFirstBack {
            cancellation: Some(cancellation(9)),
            left_closed: false,
            right_closed: false,
            decided: false,
        }
    }

    #[test]
    fn left_and_right_winners_emit_once_and_cancel_only_the_loser() {
        for (inputs, winner_port, cancellation_port, expected) in [
            ([Some(value(1)), None, None], PortId(0), PortId(2), value(1)),
            ([None, Some(value(2)), None], PortId(1), PortId(1), value(2)),
        ] {
            let mut operation = operation();
            let mut io = StepIo::test_frame(
                inputs,
                [false; 3],
                [
                    Some(conduit_core::SCALAR_ENCODED_LEN as u32),
                    Some(0),
                    Some(0),
                ],
                None,
                8,
            );
            assert_eq!(
                operation.step(&mut io, &StepInputBytes::test_frame([None; 3], None)),
                StepOutcome::Progress
            );
            assert!(io.test_consumed(winner_port));
            assert_eq!(io.test_output(PortId(0)), Some(expected));
            assert_eq!(io.test_output(cancellation_port), Some(cancellation(9)));
            let other_cancel = if cancellation_port == PortId(1) {
                PortId(2)
            } else {
                PortId(1)
            };
            assert_eq!(io.test_output(other_cancel), None);

            let mut after = StepIo::test_frame([None; 3], [false; 3], [None; 3], None, 8);
            assert_eq!(
                operation.step(&mut after, &StepInputBytes::test_frame([None; 3], None)),
                StepOutcome::Complete
            );
        }
    }

    #[test]
    fn simultaneous_values_choose_left_and_discard_right_deterministically() {
        let mut operation = operation();
        let mut io = StepIo::test_frame(
            [Some(value(3)), Some(value(4)), None],
            [false; 3],
            [
                Some(conduit_core::SCALAR_ENCODED_LEN as u32),
                Some(0),
                Some(0),
            ],
            None,
            8,
        );
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([None; 3], None)),
            StepOutcome::Progress
        );
        assert_eq!(io.test_output(PortId(0)), Some(value(3)));
        assert_eq!(io.test_output(PortId(2)), Some(cancellation(9)));
        assert!(io.test_discards().contains(&Some(value(4))));
    }

    #[test]
    fn both_normal_closures_complete_without_winner_or_cancellation() {
        let mut operation = operation();
        let mut io = StepIo::test_frame([None; 3], [true, true, false], [None; 3], None, 8);
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([None; 3], None)),
            StepOutcome::Complete
        );
        assert!(io.test_consumed_closed(PortId(0)));
        assert!(io.test_consumed_closed(PortId(1)));
        assert_eq!(io.test_output(PortId(0)), None);
        assert_eq!(io.test_output(PortId(1)), None);
        assert_eq!(io.test_output(PortId(2)), None);
    }
}
