//! One immutable typed value, published only by an admitted kernel commit.
use conduit_core::{BorrowedStructuredValueValidator, StructuredInfoRefusal};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};

/// A borrowed value producer for independently selected immutable facts, such
/// as public Base configuration. This implementation creates no Kind, native
/// law witness, readiness, resource grant or Start authority. The Source/Host
/// factory must bind the exact output contract and discharge native laws.
/// Borrowed bytes must remain funded and immutable for the complete Play.
/// Preparation and execution allocate nothing; no canonical value is copied.
/// The emitting step requires two fuel units (the local action and prepared
/// output); original kernel admission must fund that work independently.
pub struct PreparedLinkedSource<'a> {
    validator: BorrowedStructuredValueValidator<'a>,
    value: &'a [u8],
    staged: bool,
    emitted: bool,
    cancelled: bool,
}
impl<'a> PreparedLinkedSource<'a> {
    pub fn prepare(
        schema: &'a [u8],
        value: &'a [u8],
        maximum_bytes: usize,
    ) -> Result<Self, StructuredInfoRefusal> {
        let validator = BorrowedStructuredValueValidator::new(schema, maximum_bytes)?;
        validator.validate(value)?;
        Ok(Self {
            validator,
            value,
            staged: false,
            emitted: false,
            cancelled: false,
        })
    }
}
impl<const P: usize> StepBack<P> for PreparedLinkedSource<'_> {
    fn step(&mut self, io: &mut StepIo<P>, _: &StepInputBytes<'_, P>) -> StepOutcome {
        self.staged = false;
        if self.cancelled {
            return refusal(FailureCode::Cancelled);
        }
        if self.emitted {
            return StepOutcome::Complete;
        }
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        if io.consume_fuel(1).is_err()
            || io
                .send_prepared(PortId(0), self.value.len() as u32)
                .is_err()
        {
            return refusal(FailureCode::InvalidLifecycle);
        }
        self.staged = true;
        StepOutcome::Progress
    }
    fn canonical_output_schema(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0)).then_some(self.validator.type_bytes())
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && self.staged && !self.cancelled && !self.emitted).then_some(self.value)
    }
    fn step_committed(&mut self) {
        if self.staged && !self.cancelled {
            self.emitted = true;
            self.staged = false;
        }
    }
    fn cancel(&mut self) {
        self.cancelled = true;
        self.staged = false;
    }
}
fn refusal(code: FailureCode) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail: 1 })
}
