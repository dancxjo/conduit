//! Kernel Back for the distributed toggle browser sink.
//!
//! `ToggleShowBack` awaits canonical Boolean values over the remote cord
//! and drives `presentation/bool` through the browser kernel.

use conduit_core::BOOL_ENCODED_LEN;
use conduit_kernel::scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
};

/// Maximum Boolean values that the toggle sink will receive (must match `lib.rs`).
pub(super) const MAXIMUM_RECEIPTS: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CapacitySeal {
    pub(super) values: (usize, usize),
    pub(super) sign: usize,
    pub(super) identity: (usize, usize, usize),
    pub(super) projections: usize,
}

pub(super) struct ToggleShowBack {
    pub(super) next: usize,
    pub(super) pending: Option<RequestId>,
}

impl ToggleShowBack {
    fn fail(detail: u16) -> StepOutcome {
        StepOutcome::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            detail,
        })
    }
}

impl<const PORTS: usize> StepOperation<PORTS> for ToggleShowBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        _input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        if self.pending.is_none() {
            if let Some(value) = io.input(PortId(0)) {
                let Ok(sequence) = u32::try_from(self.next) else {
                    return Self::fail(1);
                };
                let request = RequestId(0x8000_0000 | sequence);
                if io.consume(PortId(0)).is_err()
                    || io
                        .request_host_call(
                            request,
                            HostCallId(0),
                            BoundedValueRef::new(value, BOOL_ENCODED_LEN as u32)
                                .expect("remote Boolean was admitted at its exact byte bound"),
                        )
                        .is_err()
                {
                    return Self::fail(2);
                }
                self.pending = Some(request);
                return StepOutcome::Progress;
            }
            if io.input_closed(PortId(0)) && self.next == MAXIMUM_RECEIPTS {
                if io.consume_closed(PortId(0)).is_err() {
                    return Self::fail(3);
                }
                return StepOutcome::Complete;
            }
            return StepOutcome::Await;
        }

        if let Some((request, outcome)) = io.host_completion() {
            if self.pending == Some(request)
                && outcome.disposition == HostCallDisposition::Completed
                && outcome.output.is_none()
                && outcome.failure.is_none()
                && io.consume_host_completion().is_ok()
            {
                self.pending = None;
                self.next += 1;
                return StepOutcome::Progress;
            }
            return Self::fail(4);
        }
        StepOutcome::Await
    }
}
