//! Retain one exact Value and publish a finite number of identical Flow items.
use alloc::{vec, vec::Vec};
use conduit_core::CheckedValueContract;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueRepeatPreparationError {
    InvalidCount,
    UnboundedValue,
}

pub struct ValueRepeatBack {
    bytes: Vec<u8>,
    length: Option<usize>,
    candidate_length: Option<usize>,
    count: u16,
    published: u16,
    staged: bool,
    terminal: bool,
}
impl ValueRepeatBack {
    pub fn prepare(
        value: &CheckedValueContract,
        count: u16,
        maximum_count: u16,
    ) -> Result<Self, ValueRepeatPreparationError> {
        if count == 0 || count > maximum_count {
            return Err(ValueRepeatPreparationError::InvalidCount);
        }
        if value.maximum_bytes == 0 && value.value_kind.as_str() != conduit_core::UNIT_INFO_ID {
            return Err(ValueRepeatPreparationError::UnboundedValue);
        }
        Ok(Self {
            bytes: vec![0; value.maximum_bytes as usize],
            length: None,
            candidate_length: None,
            count,
            published: 0,
            staged: false,
            terminal: false,
        })
    }
    pub fn allocation_capacity(&self) -> usize {
        self.bytes.capacity()
    }
    pub fn published(&self) -> u16 {
        self.published
    }
}
impl<const PORTS: usize> StepBack<PORTS> for ValueRepeatBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.terminal || self.published == self.count {
            return StepOutcome::Complete;
        }
        if let Some(length) = self.length {
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            io.send_prepared(PortId(0), length as u32)
                .expect("ready exact repeat output");
            self.staged = true;
            return StepOutcome::Progress;
        }
        if let Some(reference) = io.input(PortId(0)) {
            let Some(bytes) = inputs.input(PortId(0)) else {
                return failure();
            };
            if bytes.len() != reference.byte_len as usize || bytes.len() > self.bytes.len() {
                return failure();
            }
            self.bytes[..bytes.len()].copy_from_slice(bytes);
            self.candidate_length = Some(bytes.len());
            io.consume(PortId(0)).expect("present immutable Value");
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }
    fn step_committed(&mut self) {
        if let Some(length) = self.candidate_length.take() {
            self.length = Some(length);
        }
        if self.staged {
            self.staged = false;
            self.published += 1;
        }
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && self.staged).then(|| &self.bytes[..self.length.unwrap()])
    }
    fn cancel(&mut self) {
        self.length = None;
        self.candidate_length = None;
        self.staged = false;
        self.terminal = true;
    }
}
fn failure() -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail: 956,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::ValueRef;
    fn contract() -> CheckedValueContract {
        CheckedValueContract::new(
            conduit_core::kind_id(conduit_core::TEXT_INFO_ID),
            16,
            vec![],
        )
        .unwrap()
    }
    fn frame(input: Option<&[u8]>, ready: bool) -> (StepIo<1>, StepInputBytes<'_, 1>) {
        (
            StepIo::test_frame(
                [input.map(|bytes| ValueRef {
                    slot: 0,
                    generation: 1,
                    byte_len: bytes.len() as u32,
                })],
                [false],
                [ready.then_some(16)],
                None,
                16,
            ),
            StepInputBytes::test_frame([input], None),
        )
    }
    fn commit(back: &mut ValueRepeatBack) {
        <ValueRepeatBack as StepBack<1>>::step_committed(back);
    }
    fn admit(back: &mut ValueRepeatBack) {
        let (mut io, inputs) = frame(Some(b"exact"), false);
        assert_eq!(back.step(&mut io, &inputs), StepOutcome::Progress);
        assert!(io.test_consumed(PortId(0)));
        commit(back);
    }
    #[test]
    fn exact_copies_advance_only_after_transaction_commit_and_then_close() {
        let mut back = ValueRepeatBack::prepare(&contract(), 2, 4).unwrap();
        let capacity = back.allocation_capacity();
        admit(&mut back);
        for published in 0..2 {
            let (mut io, inputs) = frame(None, false);
            assert_eq!(back.step(&mut io, &inputs), StepOutcome::Await);
            assert_eq!(back.published(), published);
            let (mut io, inputs) = frame(None, true);
            assert_eq!(back.step(&mut io, &inputs), StepOutcome::Progress);
            assert_eq!(
                <ValueRepeatBack as StepBack<1>>::prepared_output(&back, PortId(0)),
                Some(b"exact".as_slice())
            );
            assert_eq!(back.published(), published);
            commit(&mut back);
            assert_eq!(back.published(), published + 1);
        }
        let (mut io, inputs) = frame(None, true);
        assert_eq!(back.step(&mut io, &inputs), StepOutcome::Complete);
        assert_eq!(back.allocation_capacity(), capacity);
    }
    #[test]
    fn cancel_before_or_after_one_publication_discards_remaining_copies() {
        for publish_first in [false, true] {
            let mut back = ValueRepeatBack::prepare(&contract(), 2, 2).unwrap();
            admit(&mut back);
            if publish_first {
                let (mut io, inputs) = frame(None, true);
                assert_eq!(back.step(&mut io, &inputs), StepOutcome::Progress);
                commit(&mut back);
            }
            <ValueRepeatBack as StepBack<1>>::cancel(&mut back);
            let (mut io, inputs) = frame(None, true);
            assert_eq!(back.step(&mut io, &inputs), StepOutcome::Complete);
            assert_eq!(back.published(), u16::from(publish_first));
            assert!(<ValueRepeatBack as StepBack<1>>::prepared_output(&back, PortId(0)).is_none());
        }
    }
    #[test]
    fn preparation_refuses_zero_excess_count_and_unbounded_payload() {
        assert!(matches!(
            ValueRepeatBack::prepare(&contract(), 0, 2),
            Err(ValueRepeatPreparationError::InvalidCount)
        ));
        assert!(matches!(
            ValueRepeatBack::prepare(&contract(), 3, 2),
            Err(ValueRepeatPreparationError::InvalidCount)
        ));
        let unbounded =
            CheckedValueContract::new(conduit_core::kind_id("value/unbounded"), 0, vec![]).unwrap();
        assert!(matches!(
            ValueRepeatBack::prepare(&unbounded, 1, 2),
            Err(ValueRepeatPreparationError::UnboundedValue)
        ));
    }
}
