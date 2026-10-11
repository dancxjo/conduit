//! One exact admitted Value reference is forwarded before normal Flow closure.
use conduit_core::{
    KindId, PreparedStructuredValueValidator, StructuredInfoRefusal, StructuredInfoType,
    StructuredInfoTypeShape,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};
pub struct FlowSingletonBack {
    validator: Option<PreparedStructuredValueValidator>,
    leaf: Option<KindId>,
    maximum_bytes: u32,
    staged: bool,
    emitted: bool,
    cancelled: bool,
}
impl FlowSingletonBack {
    pub fn prepare(
        schema: &StructuredInfoType,
        maximum_bytes: u32,
    ) -> Result<Self, StructuredInfoRefusal> {
        if maximum_bytes == 0
            && !matches!(schema.shape(), StructuredInfoTypeShape::Leaf(kind) if kind.as_str() == "value/empty")
        {
            return Err(StructuredInfoRefusal::CanonicalEncodingTooLarge);
        }
        Ok(Self {
            validator: if matches!(schema.shape(), StructuredInfoTypeShape::Leaf(_)) {
                None
            } else {
                Some(PreparedStructuredValueValidator::new(
                    schema,
                    maximum_bytes as usize,
                )?)
            },
            leaf: if let StructuredInfoTypeShape::Leaf(kind) = schema.shape() {
                Some(kind.clone())
            } else {
                None
            },
            maximum_bytes,
            staged: false,
            emitted: false,
            cancelled: false,
        })
    }
}
impl<const PORTS: usize> StepBack<PORTS> for FlowSingletonBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        // A rejected StepIo transaction never advances this driver's committed
        // state. Staged intent is discarded before evaluating the next attempt.
        self.staged = false;
        if self.cancelled || self.emitted {
            return StepOutcome::Complete;
        }
        if io.input_closed(PortId(0)) || io.input_abnormal(PortId(0)).is_some() {
            return fail(1090);
        }
        let Some(reference) = io.input(PortId(0)) else {
            return StepOutcome::Await;
        };
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let Some(bytes) = inputs.input(PortId(0)) else {
            return fail(1091);
        };
        if bytes.len() != reference.byte_len as usize
            || bytes.len() > self.maximum_bytes as usize
            || self
                .validator
                .as_ref()
                .is_some_and(|validator| validator.validate(bytes).is_err())
            || self.leaf.as_ref().is_some_and(|kind| {
                conduit_core::validate_primitive_info(kind.as_str(), bytes).is_err()
            })
        {
            return fail(1092);
        }
        io.consume(PortId(0)).expect("present singleton Value");
        io.send(PortId(0), reference)
            .expect("admitted singleton Flow output");
        self.staged = true;
        StepOutcome::Progress
    }
    fn step_committed(&mut self) {
        self.emitted |= self.staged;
        self.staged = false;
    }
    fn cancel(&mut self) {
        self.cancelled = true;
        self.staged = false;
    }
}
fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}
#[cfg(test)]
mod tests;
