//! Exact type-preserving cadence sampling over one finite value contract.

use alloc::vec;
use alloc::vec::Vec;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};

pub struct CadenceSampleBack {
    current: Vec<u8>,
    current_len: usize,
    has_current: bool,
    candidate: Vec<u8>,
    candidate_len: Option<usize>,
    terminal: bool,
}

impl CadenceSampleBack {
    pub fn prepare(maximum_value_bytes: usize) -> Result<Self, &'static str> {
        if maximum_value_bytes == 0
            || maximum_value_bytes > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
        {
            return Err("time/sample value bound is invalid");
        }
        Ok(Self {
            current: vec![0; maximum_value_bytes],
            current_len: 0,
            has_current: false,
            candidate: vec![0; maximum_value_bytes],
            candidate_len: None,
            terminal: false,
        })
    }

    pub fn allocation_capacity(&self) -> usize {
        self.current.capacity() + self.candidate.capacity()
    }
}

impl<const PORTS: usize> StepBack<PORTS> for CadenceSampleBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.terminal {
            return fail(810);
        }
        let value = io.input(PortId(0));
        let cadence = io.input(PortId(1));
        if cadence.is_some()
            && (value.is_some() || self.candidate_len.is_some() || self.has_current)
            && !io.output_ready(PortId(0))
        {
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
            if bytes.len() != reference.byte_len as usize || crate::decode_tick(bytes).is_err() {
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
                if io.send_prepared(PortId(0), sampled.len() as u32).is_err() {
                    return fail(815);
                }
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

    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        if port != PortId(0) {
            return None;
        }
        self.candidate_len
            .map(|len| &self.candidate[..len])
            .or_else(|| {
                self.has_current
                    .then_some(&self.current[..self.current_len])
            })
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

    fn frame<'a>(
        value: Option<&'a [u8]>,
        cadence: Option<&'a [u8]>,
        ready: bool,
    ) -> (StepIo<2>, StepInputBytes<'a, 2>) {
        (
            StepIo::test_frame(
                [
                    value.map(|bytes| reference(1, bytes)),
                    cadence.map(|bytes| reference(2, bytes)),
                ],
                [false; 2],
                [
                    ready.then_some(conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32),
                    None,
                ],
                None,
                8,
            ),
            StepInputBytes::test_frame([value, cadence], None),
        )
    }

    #[test]
    fn newest_value_wins_a_cadence_tie_and_is_derived_afresh() {
        let mut operation = CadenceSampleBack::prepare(32).unwrap();
        let tick = crate::encode_tick(1);
        let (mut io, inputs) = frame(Some(b"new"), Some(&tick), true);
        assert_eq!(operation.step(&mut io, &inputs), StepOutcome::Progress);
        assert!(io.test_prepared_output().is_some());
        assert_eq!(
            <CadenceSampleBack as StepBack<2>>::prepared_output(&operation, PortId(0)),
            Some(b"new".as_slice())
        );
        <CadenceSampleBack as StepBack<2>>::step_committed(&mut operation);

        let next = crate::encode_tick(2);
        let (mut io, inputs) = frame(None, Some(&next), true);
        assert_eq!(operation.step(&mut io, &inputs), StepOutcome::Progress);
        assert!(io.test_prepared_output().is_some());
        assert_eq!(
            <CadenceSampleBack as StepBack<2>>::prepared_output(&operation, PortId(0)),
            Some(b"new".as_slice())
        );
    }

    #[test]
    fn bounds_malformed_cadence_and_cadence_close_are_distinct() {
        assert!(CadenceSampleBack::prepare(0).is_err());
        let mut operation = CadenceSampleBack::prepare(2).unwrap();
        let (mut io, inputs) = frame(Some(b"long"), None, false);
        assert!(matches!(
            operation.step(&mut io, &inputs),
            StepOutcome::Fail(_)
        ));

        let mut operation = CadenceSampleBack::prepare(8).unwrap();
        let bad = [0xff];
        let (mut io, inputs) = frame(None, Some(&bad), true);
        assert!(matches!(
            operation.step(&mut io, &inputs),
            StepOutcome::Fail(_)
        ));

        let mut io = StepIo::test_frame([None; 2], [false, true], [None; 2], None, 8);
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
            StepOutcome::Complete
        );
    }
    #[test]
    fn physical_capsules_sample_and_retain_without_growing_play_storage() {
        let quantity = conduit_core::Quantity::new(3, conduit_core::Unit::Meter).encode();
        let unit = conduit_core::Unit::Celsius.encode();
        for bytes in [quantity.as_slice(), unit.as_slice()] {
            let mut operation = CadenceSampleBack::prepare(bytes.len()).unwrap();
            let capacity = operation.allocation_capacity();
            let tick = crate::encode_tick(1);
            let (mut blocked, inputs) = frame(Some(bytes), Some(&tick), false);
            assert_eq!(operation.step(&mut blocked, &inputs), StepOutcome::Await);
            assert_eq!(blocked.test_prepared_output(), None);
            let (mut io, inputs) = frame(Some(bytes), Some(&tick), true);
            assert_eq!(operation.step(&mut io, &inputs), StepOutcome::Progress);
            assert!(io.test_prepared_output().is_some());
            assert_eq!(
                <CadenceSampleBack as StepBack<2>>::prepared_output(&operation, PortId(0)),
                Some(bytes)
            );
            <CadenceSampleBack as StepBack<2>>::step_committed(&mut operation);
            let next = crate::encode_tick(2);
            let (mut blocked, inputs) = frame(None, Some(&next), false);
            assert_eq!(operation.step(&mut blocked, &inputs), StepOutcome::Await);
            assert!(!blocked.test_consumed(PortId(1)));
            assert!(!blocked.test_consumed(PortId(0)));
            assert_eq!(blocked.test_prepared_output(), None);
            assert_eq!(operation.allocation_capacity(), capacity);
            assert_eq!(
                <CadenceSampleBack as StepBack<2>>::prepared_output(&operation, PortId(0)),
                Some(bytes)
            );
            let (mut io, inputs) = frame(None, Some(&next), true);
            assert_eq!(operation.step(&mut io, &inputs), StepOutcome::Progress);
            assert!(io.test_prepared_output().is_some());
            assert_eq!(
                <CadenceSampleBack as StepBack<2>>::prepared_output(&operation, PortId(0)),
                Some(bytes)
            );
            assert_eq!(operation.allocation_capacity(), capacity);
        }
        assert!(
            CadenceSampleBack::prepare(conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES + 1)
                .is_err()
        );
    }
}
