//! Exact type-preserving cadence sampler over one checked finite value specialization.

use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::{CheckedValueContract, FrontValueLocation, PlannedGear};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    CanonicalValue, Failure, FailureCode, PortId,
};

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::TIME_SAMPLE_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct TimeSampleBack {
    current: Vec<u8>,
    current_len: usize,
    has_current: bool,
    candidate: Vec<u8>,
    candidate_len: Option<usize>,
    terminal: bool,
}

impl<const PORTS: usize> StepBack<PORTS> for TimeSampleBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.terminal {
            return fail(810);
        }

        let value = io.input(PortId(0));
        let cadence = io.input(PortId(1));
        if cadence.is_some() && value.is_some() && !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }

        if let Some(reference) = value {
            let Some(bytes) = inputs.input(PortId(0)) else {
                return fail(811);
            };
            if bytes.len() != reference.byte_len as usize || bytes.len() > self.candidate.len() {
                return fail(812);
            }
            self.candidate[..bytes.len()].copy_from_slice(bytes);
            self.candidate_len = Some(bytes.len());
            io.consume(PortId(0)).expect("present time/sample value");
        }

        if let Some(reference) = cadence {
            let Some(bytes) = inputs.input(PortId(1)) else {
                return fail(813);
            };
            if bytes.len() != reference.byte_len as usize
                || conduit_time::decode_tick(bytes).is_err()
            {
                return fail(814);
            }
            io.consume(PortId(1)).expect("present time/sample cadence");
            let sampled = self
                .candidate_len
                .map(|len| &self.candidate[..len])
                .or_else(|| {
                    self.has_current
                        .then_some(&self.current[..self.current_len])
                });
            if let Some(sampled) = sampled {
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                let Ok(sampled) = CanonicalValue::new(sampled) else {
                    return fail(815);
                };
                io.send_canonical(PortId(0), sampled)
                    .expect("ready exact time/sample output");
            }
            return StepOutcome::Progress;
        }

        if io.input_closed(PortId(1)) {
            io.consume_closed(PortId(1))
                .expect("observed time/sample cadence closure");
            self.terminal = true;
            return StepOutcome::Complete;
        }

        if value.is_some() {
            return StepOutcome::Progress;
        }

        if io.input_closed(PortId(0)) {
            io.consume_closed(PortId(0))
                .expect("observed time/sample value closure");
            return StepOutcome::Progress;
        }
        StepOutcome::Await
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

impl TimeSampleBack {
    pub(super) fn allocation_capacity(&self) -> usize {
        self.current.capacity() + self.candidate.capacity()
    }
}

fn exact_value_contract(placement: &PlannedGear) -> Result<&CheckedValueContract, String> {
    let input = placement
        .semantic_contract
        .value_contracts()
        .iter()
        .find(|entry| entry.location == FrontValueLocation::Input(conduit_core::port_id("value")))
        .ok_or("time/sample placement has no exact input value contract")?;
    let output = placement
        .semantic_contract
        .value_contracts()
        .iter()
        .find(|entry| entry.location == FrontValueLocation::Output(conduit_core::port_id("sample")))
        .ok_or("time/sample placement has no exact output value contract")?;
    if input.contract != output.contract {
        return Err("time/sample input and output specializations differ".into());
    }
    Ok(&input.contract)
}

fn validate(placement: &PlannedGear) -> Result<&CheckedValueContract, String> {
    let value = exact_value_contract(placement)?;
    let expected =
        conduit_semantic_catalog::time_sample_semantic_contract(value).map_err(str::to_string)?;
    if value.maximum_bytes > conduit_std_offers::TIME_SAMPLE_MAXIMUM_VALUE_BYTES
        || value.maximum_bytes as usize > CanonicalValue::MAXIMUM_BYTES
        || placement.kind_id.as_str() != conduit_semantic_catalog::TIME_SAMPLE_KIND
        || placement.kind_contract_revision.as_str()
            != conduit_semantic_catalog::TIME_SAMPLE_CONTRACT_REVISION
        || placement.execution_profile_id.as_str()
            != conduit_std_offers::TIME_SAMPLE_EXECUTION_PROFILE
        || placement.implementation_id.as_str() != conduit_std_offers::TIME_SAMPLE_IMPLEMENTATION
        || placement.artifact_id.as_str() != conduit_std_offers::TIME_SAMPLE_ARTIFACT
        || placement.inputs != expected.inputs
        || placement.outputs != expected.outputs
        || placement.semantic_contract != expected.semantic_contract()
        || !placement.configuration.is_empty()
        || !placement.host_calls.is_empty()
    {
        return Err("planned time/sample identity differs from its exact specialization".into());
    }
    Ok(value)
}

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    let value = validate(placement)?;
    Ok(BackBudget {
        value_items: 1,
        value_bytes: value.maximum_bytes,
        host_requests: 0,
        sign_items: 64,
        maximum_value_bytes: value.maximum_bytes,
    })
}

fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    let maximum = validate(placement)?.maximum_bytes as usize;
    Ok(InstalledBack::TimeSample(TimeSampleBack {
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

    fn operation(maximum: usize) -> TimeSampleBack {
        TimeSampleBack {
            current: vec![0; maximum],
            current_len: 0,
            has_current: false,
            candidate: vec![0; maximum],
            candidate_len: None,
            terminal: false,
        }
    }

    fn frame<'a>(
        value: Option<(&'a [u8], u16)>,
        cadence: Option<(&'a [u8], u16)>,
        output_ready: bool,
    ) -> (StepIo<2>, StepInputBytes<'a, 2>) {
        let inputs = [
            value.map(|(bytes, slot)| reference(slot, bytes)),
            cadence.map(|(bytes, slot)| reference(slot, bytes)),
        ];
        (
            StepIo::test_frame(
                inputs,
                [false; 2],
                [output_ready.then_some(100), None],
                None,
                8,
            ),
            StepInputBytes::test_frame(
                [
                    value.map(|(bytes, _)| bytes),
                    cadence.map(|(bytes, _)| bytes),
                ],
                None,
            ),
        )
    }

    #[test]
    fn value_wins_a_simultaneous_cadence_tie_and_repeated_ticks_freshen_output() {
        let mut operation = operation(32);
        let tick = conduit_time::encode_tick(1);
        let (mut io, inputs) = frame(Some((b"new", 1)), Some((&tick, 2)), true);
        assert_eq!(operation.step(&mut io, &inputs), StepOutcome::Progress);
        assert_eq!(io.test_canonical_output().unwrap().1.as_slice(), b"new");
        <TimeSampleBack as StepBack<2>>::step_committed(&mut operation);

        let next_tick = conduit_time::encode_tick(2);
        let (mut io, inputs) = frame(None, Some((&next_tick, 3)), true);
        assert_eq!(operation.step(&mut io, &inputs), StepOutcome::Progress);
        assert_eq!(io.test_canonical_output().unwrap().1.as_slice(), b"new");
        assert!(io.test_host_request().is_none());
    }

    #[test]
    fn cadence_before_any_value_emits_nothing_and_cadence_close_completes() {
        let mut operation = operation(32);
        let tick = conduit_time::encode_tick(1);
        let (mut io, inputs) = frame(None, Some((&tick, 1)), true);
        assert_eq!(operation.step(&mut io, &inputs), StepOutcome::Progress);
        assert!(io.test_canonical_output().is_none());

        let mut io = StepIo::test_frame([None; 2], [false, true], [None; 2], None, 8);
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
            StepOutcome::Complete
        );
        assert!(io.test_consumed_closed(PortId(1)));
    }

    #[test]
    fn malformed_and_oversized_values_refuse_without_hidden_retention() {
        let mut oversized = operation(2);
        let (mut io, inputs) = frame(Some((b"long", 1)), None, false);
        assert!(matches!(
            oversized.step(&mut io, &inputs),
            StepOutcome::Fail(_)
        ));
        assert_eq!(oversized.allocation_capacity(), 4);

        let mut operation = operation(8);
        let bad_tick = [0xff];
        let (mut io, inputs) = frame(None, Some((&bad_tick, 2)), true);
        assert!(matches!(
            operation.step(&mut io, &inputs),
            StepOutcome::Fail(_)
        ));
    }
}
