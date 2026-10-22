//! Portable implementation of the installed tick/every Back. Hosts own waits.
use crate::{encode_tick, TICK_ENCODED_LEN};
use alloc::vec::Vec;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, CanonicalValue, Failure, FailureCode, HostCallDisposition, HostCallId, PortId,
    RequestId, ValueRef,
};
pub struct TickBack {
    values: Vec<ValueRef>,
    waits: Vec<ValueRef>,
    next: usize,
    pending: Option<RequestId>,
    recurring: bool,
    limit: Option<u64>,
    sequence: u64,
    cancelled: bool,
}

impl<const PORTS: usize> StepBack<PORTS> for TickBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.cancelled {
            return StepOutcome::Fail(tick_failure(FailureCode::Cancelled, 2));
        }
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request)
                || outcome.disposition != HostCallDisposition::Completed
                || outcome.output.is_some()
                || outcome.failure.is_some()
            {
                return StepOutcome::Fail(tick_failure(FailureCode::InvalidLifecycle, 2));
            }
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            let output = if self.recurring {
                let Some(next_sequence) = self.sequence.checked_add(1) else {
                    return StepOutcome::Fail(tick_failure(
                        FailureCode::IdentityCapacityExhausted,
                        1,
                    ));
                };
                if u32::try_from(next_sequence).is_err() {
                    return StepOutcome::Fail(tick_failure(
                        FailureCode::IdentityCapacityExhausted,
                        1,
                    ));
                }
                self.sequence = next_sequence;
                None
            } else {
                let Some(value) = self.values.get(self.next).copied() else {
                    return StepOutcome::Fail(tick_failure(FailureCode::InvalidLifecycle, 1));
                };
                self.next += 1;
                Some(value)
            };
            io.consume_host_completion()
                .expect("observed Tick wait completion");
            if let Some(value) = output {
                io.send(PortId(0), value).expect("ready Tick output");
            } else {
                io.send_canonical(
                    PortId(0),
                    CanonicalValue::new(&encode_tick(self.sequence - 1))
                        .expect("Tick fits derived-value bound"),
                )
                .expect("ready recurring Tick output");
            }
            self.pending = None;
            return if self.request_wait_step(io) {
                StepOutcome::Progress
            } else if self.recurring && self.limit != Some(self.sequence) {
                StepOutcome::Fail(tick_failure(FailureCode::IdentityCapacityExhausted, 1))
            } else {
                StepOutcome::Complete
            };
        }
        if self.pending.is_none() {
            return if self.request_wait_step(io) {
                StepOutcome::Progress
            } else {
                StepOutcome::Complete
            };
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = None;
        self.cancelled = true;
    }
}

impl TickBack {
    fn request_wait_step<const PORTS: usize>(&mut self, io: &mut StepIo<PORTS>) -> bool {
        if self.limit == Some(self.sequence) {
            return false;
        }
        let index = if self.recurring { 0 } else { self.next };
        let Some(wait) = self.waits.get(index).copied() else {
            return false;
        };
        let request = if self.recurring {
            let Ok(request) = u32::try_from(self.sequence) else {
                return false;
            };
            RequestId(request)
        } else {
            let Ok(request) = u32::try_from(self.next) else {
                return false;
            };
            RequestId(request)
        };
        let input = BoundedValueRef::new(wait, TICK_ENCODED_LEN)
            .expect("wait duration is exactly eight bytes");
        io.request_host_call(request, HostCallId(0), input)
            .expect("single Tick wait Host Call");
        self.pending = Some(request);
        true
    }
}

fn tick_failure(code: FailureCode, detail: u16) -> Failure {
    Failure { code, detail }
}

impl TickBack {
    pub fn allocation_capacity(&self) -> usize {
        self.values.capacity() + self.waits.capacity()
    }
}

impl TickBack {
    /// Retained values and waits must have been reserved in the kernel store.
    /// A finite source completes after exactly the supplied number of waits.
    pub fn finite(values: Vec<ValueRef>, waits: Vec<ValueRef>) -> Option<Self> {
        if values.len() != waits.len() || values.len() > crate::MAX_TICK_COUNT as usize {
            return None;
        }
        Some(Self {
            values,
            waits,
            next: 0,
            pending: None,
            recurring: false,
            limit: None,
            sequence: 0,
            cancelled: false,
        })
    }
    /// Finite derived ticks reuse one admitted wait and constant Back storage.
    /// The typed tick values are emitted through the kernel's canonical boundary.
    pub fn counted(wait: ValueRef, count: u64) -> Option<Self> {
        if count > crate::MAX_TICK_COUNT {
            return None;
        }
        let mut back = Self::recurring(wait);
        back.limit = Some(count);
        Some(back)
    }
    /// One retained wait is reused until explicit cancellation or sequence exhaustion.
    pub fn recurring(wait: ValueRef) -> Self {
        Self {
            values: Vec::new(),
            waits: alloc::vec![wait],
            next: 0,
            pending: None,
            recurring: true,
            limit: None,
            sequence: 0,
            cancelled: false,
        }
    }
}
