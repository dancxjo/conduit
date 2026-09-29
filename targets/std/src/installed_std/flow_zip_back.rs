//! One-pending-value-per-side realization of exact `flow/zip`.

use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::{
    CheckedValueContract, FrontValueLocation, PlannedGear, PreparedTuplePairEncoder,
};
use conduit_kernel::{
    scheduler::{
        AssignedAbnormalTransduction, AssignedCancellationTransduction,
        AssignedFiniteTerminalEmission, AssignedNormalCloseTransduction,
        AssignedTerminalTransduction, StepBack, StepInputBytes, StepIo, StepOutcome,
    },
    Failure, FailureCode, PortId,
};

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::FLOW_ZIP_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct FlowZipBack {
    left: Vec<u8>,
    left_len: Option<usize>,
    right: Vec<u8>,
    right_len: Option<usize>,
    candidate: Vec<u8>,
    candidate_side: Option<(usize, usize)>,
    encoder: Box<PreparedTuplePairEncoder>,
    output_staged: bool,
    finish_after_commit: bool,
    terminal: bool,
    output_maximum: u32,
}

impl<const PORTS: usize> StepBack<PORTS> for FlowZipBack {
    fn terminal_transductions(&self) -> [Option<AssignedTerminalTransduction>; PORTS] {
        let mut contracts = [None; PORTS];
        let flush =
            AssignedNormalCloseTransduction::FlushThenPropagate(AssignedFiniteTerminalEmission {
                maximum_items: 1,
                maximum_bytes: self.output_maximum,
            });
        for (input, contract) in contracts.iter_mut().enumerate().take(2) {
            *contract = Some(AssignedTerminalTransduction {
                input: PortId(input as u16),
                output: PortId(0),
                normal_close: flush,
                abnormal: AssignedAbnormalTransduction::PropagateAfterDrain,
                cancellation: AssignedCancellationTransduction::NotCancellable,
            });
        }
        contracts
    }

    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.terminal {
            return StepOutcome::Complete;
        }

        // Fault is not normal close: never emit a buffered pair as a side
        // effect of abnormal terminal truth.
        for port in [PortId(0), PortId(1)] {
            if let Some(terminal) = io.input_abnormal(port) {
                io.consume_abnormal(port)
                    .expect("present flow/zip abnormal terminal");
                self.terminal = true;
                return StepOutcome::Abnormal {
                    port: PortId(0),
                    terminal,
                };
            }
        }

        // A normal close may emit the one pair already fully formed. It never
        // waits for a future counterpart; unmatched state is discarded.
        for port in [PortId(0), PortId(1)] {
            if io.input_closed(port) {
                let pair_ready = self.left_len.is_some() && self.right_len.is_some();
                if pair_ready && !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.consume_closed(port)
                    .expect("present flow/zip normal close");
                if pair_ready {
                    let output_len = self.encode_pair().unwrap_or(0);
                    if output_len == 0 {
                        return fail(911);
                    }
                    self.output_staged = true;
                    self.finish_after_commit = true;
                    io.send_prepared(PortId(0), output_len as u32)
                        .expect("ready admitted flow/zip output");
                    return StepOutcome::Progress;
                }
                self.left_len = None;
                self.right_len = None;
                self.terminal = true;
                return StepOutcome::Complete;
            }
        }

        if self.left_len.is_some() && self.right_len.is_some() {
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            let output_len = self.encode_pair().unwrap_or(0);
            if output_len == 0 {
                return fail(912);
            }
            self.output_staged = true;
            io.send_prepared(PortId(0), output_len as u32)
                .expect("ready admitted flow/zip output");
            return StepOutcome::Progress;
        }

        for (side, port, pending, maximum) in [
            (0, PortId(0), self.left_len, self.left.len()),
            (1, PortId(1), self.right_len, self.right.len()),
        ] {
            if pending.is_some() {
                continue;
            }
            let Some(reference) = io.input(port) else {
                continue;
            };
            let Some(bytes) = inputs.input(port) else {
                return fail(913 + side as u16);
            };
            if bytes.len() != reference.byte_len as usize || bytes.len() > maximum {
                return fail(915 + side as u16);
            }
            self.candidate[..bytes.len()].copy_from_slice(bytes);
            self.candidate_side = Some((side, bytes.len()));
            io.consume(port).expect("present flow/zip input");
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }

    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && self.output_staged).then(|| {
            // The last encode length is the exact staged length. The encoder
            // exposes it through the prepared slice without allocating.
            self.encoder.encoded()
        })
    }

    fn step_committed(&mut self) {
        if let Some((side, len)) = self.candidate_side.take() {
            if side == 0 {
                self.left[..len].copy_from_slice(&self.candidate[..len]);
                self.left_len = Some(len);
            } else {
                self.right[..len].copy_from_slice(&self.candidate[..len]);
                self.right_len = Some(len);
            }
        }
        if self.output_staged {
            self.left_len = None;
            self.right_len = None;
            self.output_staged = false;
        }
        if self.finish_after_commit {
            self.finish_after_commit = false;
            self.terminal = true;
        }
    }

    fn cancel(&mut self) {
        self.left_len = None;
        self.right_len = None;
        self.candidate_side = None;
        self.terminal = true;
    }
}

impl FlowZipBack {
    fn encode_pair(&mut self) -> Option<usize> {
        let left = &self.left[..self.left_len?];
        let right = &self.right[..self.right_len?];
        self.encoder.encode(left, right).ok().map(<[u8]>::len)
    }
}

fn exact_contracts(
    placement: &PlannedGear,
) -> Result<
    (
        &CheckedValueContract,
        &CheckedValueContract,
        &CheckedValueContract,
    ),
    String,
> {
    let contracts = placement.semantic_contract.value_contracts();
    let at = |location| {
        contracts
            .iter()
            .find(|entry| entry.location == location)
            .map(|entry| &entry.contract)
    };
    Ok((
        at(FrontValueLocation::Input(conduit_core::port_id("left")))
            .ok_or("flow/zip placement has no exact left value contract")?,
        at(FrontValueLocation::Input(conduit_core::port_id("right")))
            .ok_or("flow/zip placement has no exact right value contract")?,
        at(FrontValueLocation::Output(conduit_core::port_id("paired")))
            .ok_or("flow/zip placement has no exact paired value contract")?,
    ))
}

fn validate(
    placement: &PlannedGear,
) -> Result<
    (
        &CheckedValueContract,
        &CheckedValueContract,
        &CheckedValueContract,
    ),
    String,
> {
    let (left, right, paired) = exact_contracts(placement)?;
    let expected = conduit_semantic_catalog::flow_zip_semantic_contract(left, right)
        .map_err(str::to_string)?;
    if left.maximum_bytes > conduit_std_offers::FLOW_ZIP_MAXIMUM_INPUT_BYTES
        || right.maximum_bytes > conduit_std_offers::FLOW_ZIP_MAXIMUM_INPUT_BYTES
        || placement.kind_id.as_str() != conduit_semantic_catalog::FLOW_ZIP_KIND
        || placement.kind_contract_revision.as_str()
            != conduit_semantic_catalog::FLOW_ZIP_CONTRACT_REVISION
        || placement.execution_profile_id.as_str() != conduit_std_offers::FLOW_ZIP_EXECUTION_PROFILE
        || placement.implementation_id.as_str() != conduit_std_offers::FLOW_ZIP_IMPLEMENTATION
        || placement.artifact_id.as_str() != conduit_std_offers::FLOW_ZIP_ARTIFACT
        || placement.inputs != expected.inputs
        || placement.outputs != expected.outputs
        || placement.semantic_contract != expected.semantic_contract()
        || !placement.configuration.is_empty()
        || !placement.host_calls.is_empty()
    {
        return Err("planned flow/zip identity differs from its exact specialization".into());
    }
    Ok((left, right, paired))
}

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    let (left, right, paired) = validate(placement)?;
    Ok(BackBudget {
        value_items: 3,
        value_bytes: left
            .maximum_bytes
            .checked_add(right.maximum_bytes)
            .and_then(|bytes| bytes.checked_add(paired.maximum_bytes))
            .ok_or("flow/zip prepared value budget overflows")?,
        host_requests: 0,
        sign_items: 32,
        maximum_value_bytes: paired.maximum_bytes,
    })
}

fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    let (left, right, paired) = validate(placement)?;
    let candidate_maximum = left.maximum_bytes.max(right.maximum_bytes) as usize;
    Ok(InstalledBack::FlowZip(FlowZipBack {
        left: vec![0; left.maximum_bytes as usize],
        left_len: None,
        right: vec![0; right.maximum_bytes as usize],
        right_len: None,
        candidate: vec![0; candidate_maximum],
        candidate_side: None,
        encoder: Box::new(
            PreparedTuplePairEncoder::new(
                left.value_kind.clone(),
                left.maximum_bytes,
                right.value_kind.clone(),
                right.maximum_bytes,
            )
            .map_err(|error| format!("cannot prepare flow/zip pair encoder: {error:?}"))?,
        ),
        output_staged: false,
        finish_after_commit: false,
        terminal: false,
        output_maximum: paired.maximum_bytes,
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
    use conduit_core::{kind_id, StructuredInfoValue, StructuredInfoValueShape, TEXT_INFO_ID};
    use conduit_kernel::ValueRef;

    fn operation() -> FlowZipBack {
        let left = kind_id(TEXT_INFO_ID);
        let right = kind_id("value/count");
        let encoder = PreparedTuplePairEncoder::new(left, 16, right, 8).unwrap();
        FlowZipBack {
            left: vec![0; 16],
            left_len: None,
            right: vec![0; 8],
            right_len: None,
            candidate: vec![0; 16],
            candidate_side: None,
            output_maximum: encoder.maximum_bytes(),
            encoder: Box::new(encoder),
            output_staged: false,
            finish_after_commit: false,
            terminal: false,
        }
    }

    fn reference(slot: u16, bytes: &[u8]) -> ValueRef {
        ValueRef {
            slot,
            generation: 1,
            byte_len: bytes.len() as u32,
        }
    }

    #[test]
    fn opposite_arrival_order_emits_one_exact_pair_and_clears_both_slots() {
        let mut operation = operation();
        let count = conduit_core::encode_count(7);
        let mut io = StepIo::test_frame(
            [None, Some(reference(2, &count))],
            [false; 2],
            [Some(operation.output_maximum), None],
            None,
            16,
        );
        assert_eq!(
            operation.step(
                &mut io,
                &StepInputBytes::test_frame([None, Some(&count)], None)
            ),
            StepOutcome::Progress
        );
        assert!(io.test_consumed(PortId(1)));
        <FlowZipBack as StepBack<2>>::step_committed(&mut operation);

        let mut io = StepIo::test_frame(
            [Some(reference(1, b"hello")), None],
            [false; 2],
            [Some(operation.output_maximum), None],
            None,
            16,
        );
        assert_eq!(
            operation.step(
                &mut io,
                &StepInputBytes::test_frame([Some(b"hello"), None], None)
            ),
            StepOutcome::Progress
        );
        <FlowZipBack as StepBack<2>>::step_committed(&mut operation);

        let mut io = StepIo::test_frame(
            [None, None],
            [false; 2],
            [Some(operation.output_maximum), None],
            None,
            16,
        );
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([None, None], None)),
            StepOutcome::Progress
        );
        let encoded = <FlowZipBack as StepBack<2>>::prepared_output(&operation, PortId(0)).unwrap();
        let value = StructuredInfoValue::from_canonical_bytes(encoded).unwrap();
        let StructuredInfoValueShape::Record(fields) = value.shape() else {
            panic!("zip output must be the canonical tuple")
        };
        assert!(
            matches!(fields[0].value().shape(), StructuredInfoValueShape::Leaf(bytes) if bytes == b"hello")
        );
        assert!(
            matches!(fields[1].value().shape(), StructuredInfoValueShape::Leaf(bytes) if bytes == count)
        );
        <FlowZipBack as StepBack<2>>::step_committed(&mut operation);
        assert!(operation.left_len.is_none());
        assert!(operation.right_len.is_none());
    }

    #[test]
    fn normal_close_discards_one_unmatched_value_and_uses_exact_input_contract() {
        let mut operation = operation();
        operation.left[..4].copy_from_slice(b"solo");
        operation.left_len = Some(4);
        let contracts = <FlowZipBack as StepBack<2>>::terminal_transductions(&operation);
        assert_eq!(contracts[0].unwrap().input, PortId(0));
        assert_eq!(contracts[1].unwrap().input, PortId(1));

        let mut io = StepIo::test_frame(
            [None, None],
            [true, false],
            [Some(operation.output_maximum), None],
            None,
            16,
        );
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([None, None], None)),
            StepOutcome::Complete
        );
        assert!(io.test_consumed_closed(PortId(0)));
        assert!(operation.left_len.is_none());
    }

    #[test]
    fn closed_installed_inventory_preserves_both_terminal_contracts() {
        let installed = InstalledBack::FlowZip(operation());
        let contracts = <InstalledBack as StepBack<2>>::terminal_transductions(&installed);
        assert_eq!(contracts[0].unwrap().input, PortId(0));
        assert_eq!(contracts[1].unwrap().input, PortId(1));
    }
}
