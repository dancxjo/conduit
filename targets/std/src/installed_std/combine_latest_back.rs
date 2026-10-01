//! Retained exact latest-pair realization of `state/combine-latest`.

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
    implementation_id: conduit_std_offers::COMBINE_LATEST_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct CombineLatestBack {
    left: Vec<u8>,
    left_len: Option<usize>,
    right: Vec<u8>,
    right_len: Option<usize>,
    candidate: Vec<u8>,
    candidate_side: Option<(usize, usize)>,
    candidate_closed: Option<usize>,
    closed: [bool; 2],
    encoder: Box<PreparedTuplePairEncoder>,
    output_owed: bool,
    output_staged: bool,
    finish_after_commit: bool,
    terminal: bool,
    output_maximum: u32,
}

impl<const PORTS: usize> StepBack<PORTS> for CombineLatestBack {
    fn terminal_transductions(&self) -> [Option<AssignedTerminalTransduction>; PORTS] {
        let mut contracts = [None; PORTS];
        let normal = AssignedNormalCloseTransduction::FlushThenPropagateWhenAllClose(
            AssignedFiniteTerminalEmission {
                maximum_items: 1,
                maximum_bytes: self.output_maximum,
            },
        );
        for (input, contract) in contracts.iter_mut().enumerate().take(2) {
            *contract = Some(AssignedTerminalTransduction {
                input: PortId(input as u16),
                output: PortId(0),
                normal_close: normal,
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

        // Abnormal truth is not normal closure: do not flush a normal latest
        // value merely because one input faulted.
        for port in [PortId(0), PortId(1)] {
            if let Some(terminal) = io.input_abnormal(port) {
                io.consume_abnormal(port)
                    .expect("present combine-latest abnormal terminal");
                self.terminal = true;
                return StepOutcome::Abnormal {
                    port: PortId(0),
                    terminal,
                };
            }
        }

        // Each input close is consumed independently. The retained value from
        // an early-closing input remains available until the other closes.
        for side in 0..2 {
            let port = PortId(side as u16);
            if self.closed[side] || !io.input_closed(port) {
                continue;
            }
            if self.output_owed && !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            io.consume_closed(port)
                .expect("present combine-latest normal close");
            let last = self.closed[1 - side];
            if self.output_owed {
                if self.encode_pair().is_none() {
                    return fail(931);
                }
                self.output_staged = true;
                self.candidate_closed = Some(side);
                self.finish_after_commit = last;
                io.send_prepared(PortId(0), self.encoder.encoded().len() as u32)
                    .expect("ready admitted combine-latest output");
                return StepOutcome::Progress;
            }
            if last {
                self.terminal = true;
                return StepOutcome::Complete;
            }
            self.candidate_closed = Some(side);
            return StepOutcome::Progress;
        }

        if self.output_owed {
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            if self.encode_pair().is_none() {
                return fail(932);
            }
            self.output_staged = true;
            io.send_prepared(PortId(0), self.encoder.encoded().len() as u32)
                .expect("ready admitted combine-latest output");
            return StepOutcome::Progress;
        }

        for side in 0..2 {
            if self.closed[side] {
                continue;
            }
            let port = PortId(side as u16);
            let Some(reference) = io.input(port) else {
                continue;
            };
            let Some(bytes) = inputs.input(port) else {
                return fail(933 + side as u16);
            };
            let maximum = if side == 0 {
                self.left.len()
            } else {
                self.right.len()
            };
            if bytes.len() != reference.byte_len as usize || bytes.len() > maximum {
                return fail(935 + side as u16);
            }
            self.candidate[..bytes.len()].copy_from_slice(bytes);
            self.candidate_side = Some((side, bytes.len()));
            io.consume(port).expect("present combine-latest input");
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }

    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && self.output_staged).then(|| self.encoder.encoded())
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
            self.output_owed = self.left_len.is_some() && self.right_len.is_some();
        }
        if let Some(side) = self.candidate_closed.take() {
            self.closed[side] = true;
        }
        if self.output_staged {
            self.output_staged = false;
            self.output_owed = false;
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
        self.candidate_closed = None;
        self.output_owed = false;
        self.terminal = true;
    }
}

impl CombineLatestBack {
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
            .ok_or("state/combine-latest placement has no exact left value contract")?,
        at(FrontValueLocation::Input(conduit_core::port_id("right")))
            .ok_or("state/combine-latest placement has no exact right value contract")?,
        at(FrontValueLocation::Output(conduit_core::port_id("latest")))
            .ok_or("state/combine-latest placement has no exact latest value contract")?,
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
    let (left, right, latest) = exact_contracts(placement)?;
    let expected = conduit_semantic_catalog::combine_latest_semantic_contract(left, right)
        .map_err(str::to_string)?;
    if left.maximum_bytes > conduit_std_offers::COMBINE_LATEST_MAXIMUM_INPUT_BYTES
        || right.maximum_bytes > conduit_std_offers::COMBINE_LATEST_MAXIMUM_INPUT_BYTES
        || placement.kind_id.as_str() != conduit_semantic_catalog::COMBINE_LATEST_KIND
        || placement.kind_contract_revision.as_str()
            != conduit_semantic_catalog::COMBINE_LATEST_CONTRACT_REVISION
        || placement.execution_profile_id.as_str()
            != conduit_std_offers::COMBINE_LATEST_EXECUTION_PROFILE
        || placement.implementation_id.as_str() != conduit_std_offers::COMBINE_LATEST_IMPLEMENTATION
        || placement.artifact_id.as_str() != conduit_std_offers::COMBINE_LATEST_ARTIFACT
        || placement.inputs != expected.inputs
        || placement.outputs != expected.outputs
        || placement.semantic_contract != expected.semantic_contract()
        || !placement.configuration.is_empty()
        || !placement.host_calls.is_empty()
    {
        return Err(
            "planned state/combine-latest identity differs from its exact specialization".into(),
        );
    }
    Ok((left, right, latest))
}

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    let (left, right, latest) = validate(placement)?;
    Ok(BackBudget {
        value_items: 4,
        value_bytes: left
            .maximum_bytes
            .checked_add(right.maximum_bytes)
            .and_then(|bytes| bytes.checked_add(left.maximum_bytes.max(right.maximum_bytes)))
            .and_then(|bytes| bytes.checked_add(latest.maximum_bytes))
            .ok_or("state/combine-latest prepared value budget overflows")?,
        host_requests: 0,
        sign_items: 32,
        maximum_value_bytes: latest.maximum_bytes,
    })
}

fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    let (left, right, latest) = validate(placement)?;
    let candidate_maximum = left.maximum_bytes.max(right.maximum_bytes) as usize;
    Ok(InstalledBack::CombineLatest(CombineLatestBack {
        left: vec![0; left.maximum_bytes as usize],
        left_len: None,
        right: vec![0; right.maximum_bytes as usize],
        right_len: None,
        candidate: vec![0; candidate_maximum],
        candidate_side: None,
        candidate_closed: None,
        closed: [false; 2],
        encoder: Box::new(
            PreparedTuplePairEncoder::new(
                left.value_kind.clone(),
                left.maximum_bytes,
                right.value_kind.clone(),
                right.maximum_bytes,
            )
            .map_err(|error| format!("cannot prepare combine-latest pair encoder: {error:?}"))?,
        ),
        output_owed: false,
        output_staged: false,
        finish_after_commit: false,
        terminal: false,
        output_maximum: latest.maximum_bytes,
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

    fn operation() -> CombineLatestBack {
        let encoder =
            PreparedTuplePairEncoder::new(kind_id(TEXT_INFO_ID), 16, kind_id("value/count"), 8)
                .unwrap();
        CombineLatestBack {
            left: vec![0; 16],
            left_len: None,
            right: vec![0; 8],
            right_len: None,
            candidate: vec![0; 16],
            candidate_side: None,
            candidate_closed: None,
            closed: [false; 2],
            output_maximum: encoder.maximum_bytes(),
            encoder: Box::new(encoder),
            output_owed: false,
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

    fn accept<const SIDE: usize>(operation: &mut CombineLatestBack, bytes: &[u8]) {
        let mut inputs = [None, None];
        inputs[SIDE] = Some(reference(SIDE as u16, bytes));
        let mut payloads = [None, None];
        payloads[SIDE] = Some(bytes);
        let mut io = StepIo::test_frame(
            inputs,
            [false; 2],
            [Some(operation.output_maximum), None],
            None,
            16,
        );
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame(payloads, None)),
            StepOutcome::Progress
        );
        assert!(io.test_consumed(PortId(SIDE as u16)));
        <CombineLatestBack as StepBack<2>>::step_committed(operation);
    }

    fn emitted_pair(operation: &mut CombineLatestBack) -> (Vec<u8>, Vec<u8>) {
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
        let encoded =
            <CombineLatestBack as StepBack<2>>::prepared_output(operation, PortId(0)).unwrap();
        let value = StructuredInfoValue::from_canonical_bytes(encoded).unwrap();
        let StructuredInfoValueShape::Record(fields) = value.shape() else {
            panic!("combine-latest output must be the canonical tuple")
        };
        let StructuredInfoValueShape::Leaf(left) = fields[0].value().shape() else {
            panic!("left tuple member must preserve its exact bytes")
        };
        let StructuredInfoValueShape::Leaf(right) = fields[1].value().shape() else {
            panic!("right tuple member must preserve its exact bytes")
        };
        let pair = (left.to_vec(), right.to_vec());
        <CombineLatestBack as StepBack<2>>::step_committed(operation);
        pair
    }

    #[test]
    fn waits_for_initialization_then_emits_each_replacement_with_retained_peer() {
        let mut operation = operation();
        accept::<0>(&mut operation, b"first");
        assert!(!operation.output_owed);
        let seven = conduit_core::encode_count(7);
        accept::<1>(&mut operation, &seven);
        assert_eq!(
            emitted_pair(&mut operation),
            (b"first".to_vec(), seven.to_vec())
        );

        accept::<0>(&mut operation, b"second");
        assert_eq!(
            emitted_pair(&mut operation),
            (b"second".to_vec(), seven.to_vec())
        );
    }

    #[test]
    fn early_close_retains_latest_and_last_close_completes() {
        let mut operation = operation();
        accept::<0>(&mut operation, b"left");
        let mut io = StepIo::test_frame(
            [None, None],
            [true, false],
            [Some(operation.output_maximum), None],
            None,
            16,
        );
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([None, None], None)),
            StepOutcome::Progress
        );
        assert!(io.test_consumed_closed(PortId(0)));
        <CombineLatestBack as StepBack<2>>::step_committed(&mut operation);
        assert!(operation.closed[0]);
        assert_eq!(&operation.left[..operation.left_len.unwrap()], b"left");

        let nine = conduit_core::encode_count(9);
        accept::<1>(&mut operation, &nine);
        assert_eq!(
            emitted_pair(&mut operation),
            (b"left".to_vec(), nine.to_vec())
        );

        let mut io = StepIo::test_frame(
            [None, None],
            [false, true],
            [Some(operation.output_maximum), None],
            None,
            16,
        );
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([None, None], None)),
            StepOutcome::Complete
        );
        assert!(io.test_consumed_closed(PortId(1)));
    }

    #[test]
    fn closed_inventory_preserves_both_all_close_contracts() {
        let installed = InstalledBack::CombineLatest(operation());
        let contracts = <InstalledBack as StepBack<2>>::terminal_transductions(&installed);
        for (side, contract) in contracts.into_iter().enumerate() {
            let contract = contract.unwrap();
            assert_eq!(contract.input, PortId(side as u16));
            assert!(matches!(
                contract.normal_close,
                AssignedNormalCloseTransduction::FlushThenPropagateWhenAllClose(_)
            ));
        }
    }
}
