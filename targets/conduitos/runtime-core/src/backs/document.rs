//! Canonical initialized document output, followed only by real committed edits.
//! Domain ownership and semantic decoding remain in the admitted protected Back.
use super::{StreamTransformBack, fail};
use crate::PORTS;
use conduit_kernel::{
    PortId, ValueRef,
    scheduler::{StepIo, StepOutcome},
};

// These are the existing profile's 256-byte text plus canonical headers. Root
// validates semantic contracts before numeric admission; no capacity is widened.
const MAX_EDIT_BYTES: u32 = 16 + 256;
const MAX_DOCUMENT_BYTES: u32 = 8 + 256;

#[derive(Clone, Copy)]
pub struct DocumentBack {
    transform: StreamTransformBack,
}
impl DocumentBack {
    /// Admission owns the exact revision-zero empty snapshot before Play.
    pub(crate) fn new(initial: ValueRef) -> Self {
        let mut transform = StreamTransformBack::new(false);
        transform.initial = Some(initial);
        Self { transform }
    }
    pub(super) fn step(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        if self.transform.initial.is_some() {
            return self.transform.step(io);
        }
        if self.transform.pending.is_some() {
            if let Some((_, outcome)) = io.host_completion()
                && outcome.output.is_some_and(|output| {
                    !(8..=MAX_DOCUMENT_BYTES).contains(&output.value.byte_len)
                })
            {
                return fail(71);
            }
        } else if let Some(input) = io.input(PortId(0))
            && !(16..=MAX_EDIT_BYTES).contains(&input.byte_len)
        {
            // In particular, an empty sentinel is never an admitted edit.
            return fail(71);
        }
        self.transform.step(io)
    }
    pub(super) fn cancel(&mut self) {
        self.transform.pending = None;
        self.transform.initial = None;
    }
}

#[cfg(test)]
#[path = "document_tests.rs"]
mod tests;
