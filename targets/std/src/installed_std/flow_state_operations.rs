use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{PlannedGear, PortDescriptor, PortDirection, SCALAR_ENCODED_LEN};
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    Failure, FailureCode, PortId, ValueRef,
};

pub(super) static STATE_LATEST_SCALAR_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::STATE_LATEST_SCALAR_IMPLEMENTATION,
    budget: state_latest_budget,
    prepare: prepare_state_latest,
};

pub(super) static FLOW_TEE_SCALAR_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::FLOW_TEE_SCALAR_IMPLEMENTATION,
    budget: flow_tee_budget,
    prepare: prepare_flow_tee,
};

pub(super) struct StateLatestScalarOperation {
    held: Option<ValueRef>,
    released: Option<ValueRef>,
    retain_resumed: bool,
}

pub(super) struct FlowTeeScalarOperation {
    pending: Option<ValueRef>,
    phase: u8,
}

impl<const PORTS: usize> StepOperation<PORTS> for StateLatestScalarOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some(value) = io.input(PortId(0)) {
            if value.byte_len != SCALAR_ENCODED_LEN as u32 {
                return StepOutcome::Fail(flow_failure(12));
            }
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            let value = io.take_input(PortId(0)).expect("present latest input");
            if let Some(previous) = self.held.replace(value) {
                io.discard(previous).expect("one retained latest value");
            }
            io.send(PortId(0), value).expect("ready latest output");
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) {
            io.consume_closed(PortId(0))
                .expect("observed latest input closure");
            if let Some(held) = self.held.take() {
                io.discard(held).expect("one retained latest value");
            }
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.held = None;
        self.released = None;
        self.retain_resumed = false;
    }
}

impl<const PORTS: usize> StepOperation<PORTS> for FlowTeeScalarOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some(value) = io.input(PortId(0)) {
            if value.byte_len != SCALAR_ENCODED_LEN as u32 {
                return StepOutcome::Fail(flow_failure(13));
            }
            if !io.output_ready(PortId(0)) || !io.output_ready(PortId(1)) {
                return StepOutcome::Await;
            }
            io.consume(PortId(0)).expect("present tee input");
            io.send(PortId(0), value).expect("ready first tee output");
            io.send(PortId(1), value).expect("ready second tee output");
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) {
            io.consume_closed(PortId(0))
                .expect("observed tee input closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = None;
        self.phase = 0;
    }
}

fn flow_failure(detail: u16) -> Failure {
    Failure {
        code: FailureCode::InvalidLifecycle,
        detail,
    }
}

impl StateLatestScalarOperation {}

impl FlowTeeScalarOperation {}

fn state_latest_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_state_latest(placement)?;
    Ok(budget())
}

fn flow_tee_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_flow_tee(placement)?;
    Ok(budget())
}

fn budget() -> OperationBudget {
    OperationBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 0,
        sign_items: 96,
        maximum_value_bytes: SCALAR_ENCODED_LEN as u32,
    }
}

fn prepare_state_latest(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_state_latest(placement)?;
    Ok(InstalledOperation::StateLatestScalar(
        StateLatestScalarOperation {
            held: None,
            released: None,
            retain_resumed: false,
        },
    ))
}

fn prepare_flow_tee(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_flow_tee(placement)?;
    Ok(InstalledOperation::FlowTeeScalar(FlowTeeScalarOperation {
        pending: None,
        phase: 0,
    }))
}

fn validate_state_latest(placement: &PlannedGear) -> Result<(), String> {
    validate_identity(
        placement,
        InstalledIdentity {
            kind: conduit_semantic_catalog::LATEST_KIND,
            revision: conduit_semantic_catalog::STATE_LATEST_SCALAR_CONTRACT_REVISION,
            profile: conduit_std_offers::STATE_LATEST_SCALAR_EXECUTION_PROFILE,
            implementation: conduit_std_offers::STATE_LATEST_SCALAR_IMPLEMENTATION,
            artifact: conduit_std_offers::STATE_LATEST_SCALAR_ARTIFACT,
        },
        &conduit_semantic_catalog::state_latest_scalar_contract().inputs,
        &conduit_semantic_catalog::state_latest_scalar_contract().outputs,
    )
}

fn validate_flow_tee(placement: &PlannedGear) -> Result<(), String> {
    validate_identity(
        placement,
        InstalledIdentity {
            kind: conduit_semantic_catalog::TEE_KIND,
            revision: conduit_semantic_catalog::FLOW_TEE_SCALAR_CONTRACT_REVISION,
            profile: conduit_std_offers::FLOW_TEE_SCALAR_EXECUTION_PROFILE,
            implementation: conduit_std_offers::FLOW_TEE_SCALAR_IMPLEMENTATION,
            artifact: conduit_std_offers::FLOW_TEE_SCALAR_ARTIFACT,
        },
        &conduit_semantic_catalog::flow_tee_scalar_contract().inputs,
        &conduit_semantic_catalog::flow_tee_scalar_contract().outputs,
    )
}

struct InstalledIdentity {
    kind: &'static str,
    revision: &'static str,
    profile: &'static str,
    implementation: &'static str,
    artifact: &'static str,
}

fn validate_identity(
    placement: &PlannedGear,
    identity: InstalledIdentity,
    inputs: &[PortDescriptor],
    outputs: &[PortDescriptor],
) -> Result<(), String> {
    if placement.kind_id.as_str() != identity.kind
        || placement.kind_contract_revision.as_str() != identity.revision
        || placement.execution_profile_id.as_str() != identity.profile
        || placement.implementation_id.as_str() != identity.implementation
        || placement.artifact_id.as_str() != identity.artifact
        || placement.inputs != inputs
        || placement.outputs != outputs
        || !placement.configuration.is_empty()
        || inputs
            .iter()
            .any(|port| port.direction != PortDirection::Input)
        || outputs
            .iter()
            .any(|port| port.direction != PortDirection::Output)
    {
        return Err("planned flow/state scalar identity does not match its installation".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(slot: u16) -> ValueRef {
        ValueRef {
            slot,
            generation: 1,
            byte_len: SCALAR_ENCODED_LEN as u32,
        }
    }

    #[test]
    fn latest_replaces_one_retained_value_and_releases_on_close() {
        let mut operation = StateLatestScalarOperation {
            held: None,
            released: None,
            retain_resumed: false,
        };
        let mut io = StepIo::test_frame(
            [Some(value(1))],
            [false],
            [Some(SCALAR_ENCODED_LEN as u32)],
            None,
            8,
        );
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Progress
        );
        assert!(io.test_retained(PortId(0)));
        assert_eq!(io.test_output(PortId(0)), Some(value(1)));

        let mut io = StepIo::test_frame(
            [Some(value(2))],
            [false],
            [Some(SCALAR_ENCODED_LEN as u32)],
            None,
            8,
        );
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Progress
        );
        assert!(io.test_retained(PortId(0)));
        assert!(io.test_discards().contains(&Some(value(1))));
        assert_eq!(io.test_output(PortId(0)), Some(value(2)));

        let mut io = StepIo::test_frame([None], [true], [None], None, 8);
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Complete
        );
        assert!(io.test_consumed_closed(PortId(0)));
        assert!(io.test_discards().contains(&Some(value(2))));
    }

    #[test]
    fn tee_exposes_both_outputs_in_one_operation_transaction() {
        let mut operation = FlowTeeScalarOperation {
            pending: None,
            phase: 0,
        };
        let mut io = StepIo::test_frame(
            [Some(value(3)), None],
            [false; 2],
            [Some(SCALAR_ENCODED_LEN as u32); 2],
            None,
            8,
        );
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([None, None], None)),
            StepOutcome::Progress
        );
        assert!(io.test_consumed(PortId(0)));
        assert_eq!(io.test_output(PortId(0)), Some(value(3)));
        assert_eq!(io.test_output(PortId(1)), Some(value(3)));
    }
}
