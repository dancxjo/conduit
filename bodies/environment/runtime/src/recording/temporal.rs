//! Existing flow/singleton meaning, specialized to bounded JSON snapshot text.
//! This changes temporal shape only; it performs no read or content recovery.
use super::MAXIMUM_CHUNK_BYTES;
use conduit_core as c;
use conduit_kernel::{scheduler::*, Failure, FailureCode, PortId};
pub fn recording_singleton_kind() -> c::Kind {
    let kind = c::kind_id("text/json-utf8@1");
    let value =
        c::CheckedValueContract::new(kind.clone(), MAXIMUM_CHUNK_BYTES as u32, alloc::vec![])
            .expect("finite JSON envelope");
    let schema = c::StructuredInfoType::leaf(kind).expect("JSON leaf");
    conduit_semantic_catalog::flow_singleton_semantic_contract(&value, &schema)
        .expect("matching JSON schema")
}
#[derive(Default)]
pub struct RecordingSingletonBack {
    staged: bool,
    complete: bool,
    cancelled: bool,
}
impl<const P: usize> StepBack<P> for RecordingSingletonBack {
    fn step(&mut self, io: &mut StepIo<P>, input: &StepInputBytes<'_, P>) -> StepOutcome {
        self.staged = false;
        let fail = |code, detail| StepOutcome::Fail(Failure { code, detail });
        if self.cancelled {
            return fail(FailureCode::Cancelled, 1);
        }
        if self.complete {
            return fail(FailureCode::InvalidLifecycle, 2);
        }
        if io.input_abnormal(PortId(0)).is_some() {
            return fail(FailureCode::InvalidInput, 3);
        }
        let Some(value) = io.input(PortId(0)) else {
            return if io.input_closed(PortId(0)) {
                fail(FailureCode::InvalidInput, 4)
            } else {
                StepOutcome::Await
            };
        };
        let Some(bytes) = input.input(PortId(0)) else {
            return fail(FailureCode::InvalidInput, 5);
        };
        if value.byte_len > MAXIMUM_CHUNK_BYTES as u32
            || bytes.len() != value.byte_len as usize
            || bytes.len() > MAXIMUM_CHUNK_BYTES
        {
            return fail(FailureCode::InvalidInput, 6);
        }
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        if io.send(PortId(0), value).is_err() || io.consume(PortId(0)).is_err() {
            return fail(FailureCode::InvalidLifecycle, 7);
        }
        self.staged = true;
        StepOutcome::Complete
    }
    fn step_committed(&mut self) {
        if self.staged {
            self.complete = true;
            self.staged = false;
        }
    }
    fn cancel(&mut self) {
        self.cancelled = true;
        self.staged = false;
    }
}
