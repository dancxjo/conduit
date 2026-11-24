//! Home observation sink over the existing admitted Face commit/custody boundary.
//!
//! Registration requires the reviewed generic IMAGE/Face foundation from N.
//! This module is intentionally not registered before that dependency lands.
use crate::body_execution::face_contribution::{FaceCommitGate, FaceCommitObservation};
use conduit_core::PlacementId;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
    ValueRef,
};

const INPUT: PortId = PortId(0);
const PROJECT: HostCallId = HostCallId(0);
const ACCEPT_CUSTODY: HostCallId = HostCallId(1);
#[cfg(test)]
const INPUT_BYTES: u32 = conduit_semantic_catalog::AMBIENT_HOME_OBSERVATION_BYTES;

#[derive(Clone, Copy)]
struct Pending {
    request: RequestId,
    value: ValueRef,
    custody: bool,
}
enum Candidate {
    Pending(Pending),
    Accepted,
    Retired,
}

pub(in crate::installed_std) struct AmbientHomeBack {
    gate: FaceCommitGate,
    maximum_input_bytes: u32,
    maximum_updates: Option<u32>,
    accepted_updates: u32,
    pending: Option<Pending>,
    candidate: Option<Candidate>,
    cancelled: bool,
}

impl AmbientHomeBack {
    /// The installed factory must first check the exact planned offer, and pay
    /// its request, Sign, value and projection-workspace bounds. No quota is
    /// inferred from a measurement provider or an unbounded input stream here.
    #[cfg(test)]
    pub(in crate::installed_std) fn prepare(
        placement: PlacementId,
        maximum_updates: u32,
    ) -> Result<Self, String> {
        if maximum_updates == 0 || maximum_updates.checked_mul(2).is_none() {
            return Err("Home requires a finite two-call update scope".into());
        }
        Self::prepare_limit(placement, Some(maximum_updates), INPUT_BYTES)
    }
    pub(in crate::installed_std) fn prepare_resident(
        placement: PlacementId,
        maximum_input_bytes: u32,
    ) -> Result<Self, String> {
        Self::prepare_limit(placement, None, maximum_input_bytes)
    }
    fn prepare_limit(
        placement: PlacementId,
        maximum_updates: Option<u32>,
        maximum_input_bytes: u32,
    ) -> Result<Self, String> {
        let gate = match maximum_updates {
            Some(maximum) => FaceCommitGate::prepare(placement, PROJECT, maximum),
            None => FaceCommitGate::prepare_resident(placement, PROJECT),
        }
        .map_err(|why| format!("Home Face gate: {why:?}"))?;
        Ok(Self {
            gate,
            maximum_input_bytes,
            maximum_updates,
            accepted_updates: 0,
            pending: None,
            candidate: None,
            cancelled: false,
        })
    }

    pub(in crate::installed_std) fn observation(&self) -> FaceCommitObservation {
        self.gate.observation()
    }

    pub(in crate::installed_std) fn retained_backing_bytes(&self) -> Option<usize> {
        self.gate.prepared_backing_bytes()
    }
}

impl<const PORTS: usize> StepBack<PORTS> for AmbientHomeBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        self.candidate = None;
        self.gate.begin_step();
        if self.cancelled {
            return failed(FailureCode::InvalidLifecycle);
        }
        if self.pending.is_none() && io.host_completion().is_some() {
            return failed(FailureCode::InvalidLifecycle);
        }
        if let Some(pending) = self.pending {
            if io.input(INPUT) != Some(pending.value) {
                return failed(FailureCode::InvalidLifecycle);
            }
            let Some((request, outcome)) = io.host_completion() else {
                return StepOutcome::Await;
            };
            if request != pending.request {
                return failed(FailureCode::InvalidLifecycle);
            }
            if pending.custody {
                if outcome.disposition != HostCallDisposition::Completed
                    || outcome.output.is_some()
                    || outcome.failure.is_some()
                    || io.consume_host_completion().is_err()
                    || io.consume(INPUT).is_err()
                {
                    return failed(FailureCode::InvalidLifecycle);
                }
                self.candidate = Some(Candidate::Accepted);
            } else {
                if self.gate.consume_completion(io, request).is_err() {
                    return failed(FailureCode::InvalidLifecycle);
                }
                let Some(next) = request.0.checked_add(1).map(RequestId) else {
                    return failed(FailureCode::StorageExhausted);
                };
                let Ok(input) = BoundedValueRef::new(pending.value, self.maximum_input_bytes)
                else {
                    return failed(FailureCode::InvalidInput);
                };
                if io.borrow_input_for_call(INPUT).is_err()
                    || io.request_host_call(next, ACCEPT_CUSTODY, input).is_err()
                {
                    return failed(FailureCode::InvalidLifecycle);
                }
                self.candidate = Some(Candidate::Pending(Pending {
                    request: next,
                    custody: true,
                    ..pending
                }));
            }
            return StepOutcome::Progress;
        }
        if let Some(value) = io.input(INPUT) {
            if self
                .maximum_updates
                .is_some_and(|maximum| self.accepted_updates >= maximum)
            {
                return failed(FailureCode::StorageExhausted);
            }
            let Some(request) = self.accepted_updates.checked_mul(2).map(RequestId) else {
                return failed(if self.maximum_updates.is_none() {
                    FailureCode::IdentityCapacityExhausted
                } else {
                    FailureCode::StorageExhausted
                });
            };
            let Ok(input) = BoundedValueRef::new(value, self.maximum_input_bytes) else {
                return failed(FailureCode::InvalidInput);
            };
            if io.borrow_input_for_call(INPUT).is_err()
                || io.request_host_call(request, PROJECT, input).is_err()
            {
                return failed(FailureCode::InvalidLifecycle);
            }
            self.candidate = Some(Candidate::Pending(Pending {
                request,
                value,
                custody: false,
            }));
            return StepOutcome::Progress;
        }
        if io.input_closed(INPUT) {
            if io.consume_closed(INPUT).is_err() {
                return failed(FailureCode::InvalidLifecycle);
            }
            self.candidate = Some(Candidate::Retired);
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn step_committed(&mut self) {
        self.gate.step_committed();
        match self.candidate.take() {
            Some(Candidate::Pending(pending)) => self.pending = Some(pending),
            Some(Candidate::Accepted) => {
                self.pending = None;
                self.accepted_updates += 1;
            }
            Some(Candidate::Retired) => self.gate.cancel(),
            None => {}
        }
    }

    fn cancel(&mut self) {
        self.cancelled = true;
        self.pending = None;
        self.candidate = None;
        self.gate.cancel();
    }
}

fn failed(code: FailureCode) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail: 81 })
}

#[cfg(test)]
#[path = "ambient_home_back/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "ambient_home_back/kernel_tests.rs"]
mod kernel_tests;
