//! One-shot snapshot of one exact current-value specialization.

use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::{CheckedValueContract, FrontValueLocation, PlannedGear};
use conduit_kernel::{
    scheduler::{
        AssignedAbnormalTransduction, AssignedCancellationTransduction,
        AssignedNormalCloseTransduction, AssignedTerminalTransduction, StepBack, StepInputBytes,
        StepIo, StepOutcome,
    },
    Failure, FailureCode, PortId,
};

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::STATE_SNAPSHOT_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct StateSnapshotBack {
    current: Vec<u8>,
    current_len: usize,
    has_current: bool,
    candidate: Vec<u8>,
    candidate_len: Option<usize>,
    terminal: bool,
}

impl<const PORTS: usize> StepBack<PORTS> for StateSnapshotBack {
    fn terminal_transduction(&self) -> Option<AssignedTerminalTransduction> {
        Some(AssignedTerminalTransduction {
            input: PortId(1),
            output: PortId(0),
            normal_close: AssignedNormalCloseTransduction::NotAccepted,
            abnormal: AssignedAbnormalTransduction::PropagateAfterDrain,
            cancellation: AssignedCancellationTransduction::NotCancellable,
        })
    }

    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.terminal {
            return StepOutcome::Complete;
        }
        // A replacement presented with the trigger wins the tie. Commit it in
        // this transaction and leave the trigger for the next, so output never
        // exposes uncommitted private state.
        if let Some(reference) = io.input(PortId(0)) {
            let Some(bytes) = inputs.input(PortId(0)) else {
                return fail(901);
            };
            if bytes.len() != reference.byte_len as usize || bytes.len() > self.candidate.len() {
                return fail(902);
            }
            self.candidate[..bytes.len()].copy_from_slice(bytes);
            self.candidate_len = Some(bytes.len());
            io.consume(PortId(0))
                .expect("present state/snapshot current");
            return StepOutcome::Progress;
        }
        if let Some(terminal) = io.input_abnormal(PortId(1)) {
            io.consume_abnormal(PortId(1))
                .expect("present state/snapshot trigger terminal");
            self.terminal = true;
            return StepOutcome::Abnormal {
                port: PortId(0),
                terminal,
            };
        }
        if let Some(reference) = io.input(PortId(1)) {
            let Some(trigger) = inputs.input(PortId(1)) else {
                return fail(903);
            };
            if reference.byte_len != 0 || !trigger.is_empty() || !self.has_current {
                return fail(904);
            }
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            io.consume(PortId(1))
                .expect("present state/snapshot trigger");
            io.send_prepared(PortId(0), self.current_len as u32)
                .expect("ready admitted state/snapshot output");
            self.terminal = true;
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }

    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && self.terminal && self.has_current)
            .then_some(&self.current[..self.current_len])
    }

    fn step_committed(&mut self) {
        if let Some(len) = self.candidate_len.take() {
            self.current[..len].copy_from_slice(&self.candidate[..len]);
            self.current_len = len;
            self.has_current = true;
        }
    }

    fn cancel(&mut self) {
        self.current_len = 0;
        self.has_current = false;
        self.candidate_len = None;
        self.terminal = true;
    }
}

fn exact_value_contract(placement: &PlannedGear) -> Result<&CheckedValueContract, String> {
    let contracts = placement.semantic_contract.value_contracts();
    let current = contracts
        .iter()
        .find(|entry| entry.location == FrontValueLocation::Input(conduit_core::port_id("current")))
        .ok_or("state/snapshot placement has no exact current value contract")?;
    let output = contracts
        .iter()
        .find(|entry| entry.location == FrontValueLocation::Output(conduit_core::port_id("value")))
        .ok_or("state/snapshot placement has no exact output value contract")?;
    if current.contract != output.contract {
        return Err("state/snapshot current and output specializations differ".into());
    }
    Ok(&current.contract)
}

fn validate(placement: &PlannedGear) -> Result<&CheckedValueContract, String> {
    let value = exact_value_contract(placement)?;
    let expected = conduit_semantic_catalog::state_snapshot_semantic_contract(value)
        .map_err(str::to_string)?;
    if value.maximum_bytes > conduit_std_offers::STATE_SNAPSHOT_MAXIMUM_VALUE_BYTES
        || placement.kind_id.as_str() != conduit_semantic_catalog::STATE_SNAPSHOT_KIND
        || placement.kind_contract_revision.as_str()
            != conduit_semantic_catalog::STATE_SNAPSHOT_CONTRACT_REVISION
        || placement.execution_profile_id.as_str()
            != conduit_std_offers::STATE_SNAPSHOT_EXECUTION_PROFILE
        || placement.implementation_id.as_str() != conduit_std_offers::STATE_SNAPSHOT_IMPLEMENTATION
        || placement.artifact_id.as_str() != conduit_std_offers::STATE_SNAPSHOT_ARTIFACT
        || placement.inputs != expected.inputs
        || placement.outputs != expected.outputs
        || placement.semantic_contract != expected.semantic_contract()
        || !placement.configuration.is_empty()
        || !placement.host_calls.is_empty()
    {
        return Err("planned state/snapshot identity differs from its exact specialization".into());
    }
    Ok(value)
}

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    let value = validate(placement)?;
    Ok(BackBudget {
        value_items: 1,
        value_bytes: value.maximum_bytes,
        host_requests: 0,
        sign_items: 32,
        maximum_value_bytes: value.maximum_bytes,
    })
}

fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    let maximum = validate(placement)?.maximum_bytes as usize;
    Ok(InstalledBack::StateSnapshot(StateSnapshotBack {
        current: vec![0; maximum],
        current_len: 0,
        has_current: false,
        candidate: vec![0; maximum],
        candidate_len: None,
        terminal: false,
    }))
}

fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::ValueRef;

    fn reference(slot: u16, bytes: &[u8]) -> ValueRef {
        ValueRef {
            slot,
            generation: 1,
            byte_len: bytes.len() as u32,
        }
    }

    #[test]
    fn current_update_commits_before_one_unit_trigger_stages_the_exact_large_value() {
        let mut operation = StateSnapshotBack {
            current: vec![0; 4_096],
            current_len: 0,
            has_current: false,
            candidate: vec![0; 4_096],
            candidate_len: None,
            terminal: false,
        };
        let current = vec![0x5a; 4_096];
        let mut io = StepIo::test_frame(
            [Some(reference(1, &current)), Some(reference(2, &[]))],
            [false; 2],
            [Some(4_096), None],
            None,
            8,
        );
        let inputs = StepInputBytes::test_frame([Some(&current), Some(&[])], None);
        assert_eq!(operation.step(&mut io, &inputs), StepOutcome::Progress);
        assert!(io.test_consumed(PortId(0)));
        assert!(!io.test_consumed(PortId(1)));
        <StateSnapshotBack as StepBack<2>>::step_committed(&mut operation);

        let mut io = StepIo::test_frame(
            [None, Some(reference(2, &[]))],
            [false; 2],
            [Some(4_096), None],
            None,
            8,
        );
        let inputs = StepInputBytes::test_frame([None, Some(&[])], None);
        assert_eq!(operation.step(&mut io, &inputs), StepOutcome::Progress);
        assert_eq!(io.test_prepared_output(), Some((PortId(0), 4_096)));
        assert_eq!(
            <StateSnapshotBack as StepBack<2>>::prepared_output(&operation, PortId(0)),
            Some(current.as_slice())
        );
    }
}
