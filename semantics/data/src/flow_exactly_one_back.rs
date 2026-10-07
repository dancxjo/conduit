//! Exact-schema closing Flow to one immutable Value; zero/multiple items refuse.
use alloc::{vec, vec::Vec};
use conduit_core::{
    CheckedValueContract, PreparedStructuredValueValidator, StructuredInfoType,
    StructuredInfoTypeShape,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowExactlyOnePreparationError {
    InvalidSchema,
}
enum Validator {
    Primitive(CheckedValueContract),
    Structured(PreparedStructuredValueValidator),
}

pub struct FlowExactlyOneBack {
    validator: Validator,
    bytes: Vec<u8>,
    length: Option<usize>,
    candidate: Option<usize>,
    closed: bool,
    close_staged: bool,
    output_staged: bool,
    terminal: bool,
}
impl FlowExactlyOneBack {
    pub fn prepare(
        value: &CheckedValueContract,
        schema: &StructuredInfoType,
    ) -> Result<Self, FlowExactlyOnePreparationError> {
        use FlowExactlyOnePreparationError::InvalidSchema;
        value.validate_definition().map_err(|_| InvalidSchema)?;
        let validator = match schema.shape() {
            StructuredInfoTypeShape::Leaf(kind) if kind == &value.value_kind => {
                Validator::Primitive(value.clone())
            }
            StructuredInfoTypeShape::Leaf(_) => return Err(InvalidSchema),
            _ => {
                if schema.profile().map_err(|_| InvalidSchema)?.value_kind() != &value.value_kind
                    || !value.constraints.is_empty()
                {
                    return Err(InvalidSchema);
                }
                Validator::Structured(
                    PreparedStructuredValueValidator::new(schema, value.maximum_bytes as usize)
                        .map_err(|_| InvalidSchema)?,
                )
            }
        };

        Ok(Self {
            validator,
            bytes: vec![0; value.maximum_bytes as usize],
            length: None,
            candidate: None,
            closed: false,
            close_staged: false,
            output_staged: false,
            terminal: false,
        })
    }
    pub fn allocation_capacity(&self) -> usize {
        self.bytes.capacity()
    }
}
impl<const PORTS: usize> StepBack<PORTS> for FlowExactlyOneBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.terminal {
            return StepOutcome::Complete;
        }
        if self.closed {
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            let Some(length) = self.length else {
                return failure();
            };
            io.send_prepared(PortId(0), length as u32)
                .expect("ready exact singleton Value");
            self.output_staged = true;
            return StepOutcome::Progress;
        }
        if let Some(reference) = io.input(PortId(0)) {
            if self.length.is_some() || self.candidate.is_some() {
                return failure();
            }
            let Some(bytes) = inputs.input(PortId(0)) else {
                return failure();
            };
            if bytes.len() != reference.byte_len as usize || bytes.len() > self.bytes.len() {
                return failure();
            }
            let valid = match &self.validator {
                Validator::Primitive(value) => value.validate(bytes).is_ok(),
                Validator::Structured(schema) => schema.validate(bytes).is_ok(),
            };
            if !valid {
                return failure();
            }
            self.bytes[..bytes.len()].copy_from_slice(bytes);
            self.candidate = Some(bytes.len());
            io.consume(PortId(0)).expect("present exact singleton item");
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) {
            if self.length.is_none() {
                return failure();
            }
            io.consume_closed(PortId(0))
                .expect("present exact singleton closure");
            self.close_staged = true;
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }
    fn step_committed(&mut self) {
        if let Some(length) = self.candidate.take() {
            self.length = Some(length);
        }
        if self.close_staged {
            self.close_staged = false;
            self.closed = true;
        }
        if self.output_staged {
            self.output_staged = false;
            self.terminal = true;
        }
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && self.output_staged)
            .then(|| &self.bytes[..self.length.expect("retained singleton")])
    }
    fn cancel(&mut self) {
        self.length = None;
        self.candidate = None;
        self.close_staged = false;
        self.output_staged = false;
        self.closed = false;
        self.terminal = true;
    }
}
fn failure() -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail: 957,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::ValueRef;
    fn prepared() -> FlowExactlyOneBack {
        let value =
            CheckedValueContract::new(conduit_core::kind_id(conduit_core::BOOL_INFO_ID), 1, vec![])
                .unwrap();
        let schema = StructuredInfoType::leaf(value.value_kind.clone()).unwrap();
        FlowExactlyOneBack::prepare(&value, &schema).unwrap()
    }
    fn frame(
        input: Option<&[u8]>,
        closed: bool,
        ready: bool,
    ) -> (StepIo<1>, StepInputBytes<'_, 1>) {
        (
            StepIo::test_frame(
                [input.map(|bytes| ValueRef {
                    slot: 0,
                    generation: 1,
                    byte_len: bytes.len() as u32,
                })],
                [closed],
                [ready.then_some(1)],
                None,
                16,
            ),
            StepInputBytes::test_frame([input], None),
        )
    }
    fn commit(back: &mut FlowExactlyOneBack) {
        <FlowExactlyOneBack as StepBack<1>>::step_committed(back);
    }
    fn admit(back: &mut FlowExactlyOneBack) {
        let (mut io, inputs) = frame(Some(&[1]), false, true);
        assert_eq!(back.step(&mut io, &inputs), StepOutcome::Progress);
        assert!(io.test_consumed(PortId(0)));
        assert!(<FlowExactlyOneBack as StepBack<1>>::prepared_output(back, PortId(0)).is_none());
        commit(back);
    }
    #[test]
    fn normal_close_then_pressure_preserves_exact_item_until_publication_commit() {
        let mut back = prepared();
        let capacity = back.allocation_capacity();
        admit(&mut back);
        let (mut io, inputs) = frame(None, false, true);
        assert_eq!(back.step(&mut io, &inputs), StepOutcome::Await);
        let (mut io, inputs) = frame(None, true, false);
        assert_eq!(back.step(&mut io, &inputs), StepOutcome::Progress);
        commit(&mut back);
        let (mut io, inputs) = frame(None, false, false);
        assert_eq!(back.step(&mut io, &inputs), StepOutcome::Await);
        let (mut io, inputs) = frame(None, false, true);
        assert_eq!(back.step(&mut io, &inputs), StepOutcome::Progress);
        assert_eq!(
            <FlowExactlyOneBack as StepBack<1>>::prepared_output(&back, PortId(0)),
            Some([1].as_slice())
        );
        assert!(!back.terminal);
        commit(&mut back);
        assert_eq!(back.step(&mut io, &inputs), StepOutcome::Complete);
        assert_eq!(back.allocation_capacity(), capacity);
    }
    #[test]
    fn zero_two_and_malformed_items_refuse_before_consuming_invalid_input() {
        let mut empty = prepared();
        let (mut io, inputs) = frame(None, true, true);
        assert!(matches!(empty.step(&mut io, &inputs), StepOutcome::Fail(_)));
        let mut two = prepared();
        admit(&mut two);
        let (mut io, inputs) = frame(Some(&[0]), false, true);
        assert!(matches!(two.step(&mut io, &inputs), StepOutcome::Fail(_)));
        assert!(!io.test_consumed(PortId(0)));
        let mut malformed = prepared();
        let (mut io, inputs) = frame(Some(&[2]), false, true);
        assert!(matches!(
            malformed.step(&mut io, &inputs),
            StepOutcome::Fail(_)
        ));
        assert!(!io.test_consumed(PortId(0)));
        assert!(
            <FlowExactlyOneBack as StepBack<1>>::prepared_output(&malformed, PortId(0)).is_none()
        );
    }
    #[test]
    fn cancellation_discards_retained_and_staged_publication() {
        for stage in [false, true] {
            let mut back = prepared();
            admit(&mut back);
            if stage {
                let (mut io, inputs) = frame(None, true, true);
                assert_eq!(back.step(&mut io, &inputs), StepOutcome::Progress);
                commit(&mut back);
                let (mut io, inputs) = frame(None, false, true);
                assert_eq!(back.step(&mut io, &inputs), StepOutcome::Progress);
            }
            <FlowExactlyOneBack as StepBack<1>>::cancel(&mut back);
            let (mut io, inputs) = frame(None, false, true);
            assert_eq!(back.step(&mut io, &inputs), StepOutcome::Complete);
            assert!(
                <FlowExactlyOneBack as StepBack<1>>::prepared_output(&back, PortId(0)).is_none()
            );
        }
    }
}
