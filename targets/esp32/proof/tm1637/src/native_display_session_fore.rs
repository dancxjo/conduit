//! Finite reusable output invocations, with exact evidence custody between them.
use super::{fore::LocalFore, Execution, Exit, Outcome};
use conduit_kernel::{
    self as k,
    scheduler::{RemoteIngressOutcome, SchedulerStatus},
    SignSink,
};
use conduit_tm1637_wire::OpenDrain;
use core::future::Future;
use embassy_futures::{
    select::{select, Either},
    yield_now,
};
impl<P: OpenDrain> Execution<P> {
    pub async fn run_with_fore(
        &mut self,
        stop: impl Future<Output = ()>,
        fore: &mut impl LocalFore,
        maximum_frames: u32,
    ) -> Outcome {
        if let Some(outcome) = self.begin_run() {
            return outcome;
        }
        if maximum_frames == 0
            || maximum_frames > self.storage.maximum_frames()
            || maximum_frames > self.inspection().scope.maximum_operations
        {
            return self.finish_run(failed(k::FailureCode::WorkBudgetExhausted, 40));
        }
        let exit = match select(stop, self.pump_session(fore, maximum_frames)).await {
            Either::First(()) => Exit::Cancelled,
            Either::Second(exit) => exit,
        };
        self.finish_run(exit)
    }
    pub fn stage_evidence(
        &self,
    ) -> Result<Option<crate::native_owner_control::EvidenceReference>, &'static str> {
        if self.storage.kernel.signs().is_empty() {
            return Ok(None);
        }
        super::super::evidence_task::publish(self.invocation, self.storage.kernel.signs())
    }
    async fn pump_session(&mut self, fore: &mut impl LocalFore, maximum_frames: u32) -> Exit {
        let input = self.storage.input;
        let refusal = self.storage.refusal;
        for sequence in 0..u64::from(maximum_frames) {
            self.invocation = sequence;
            let frame = match fore.next().await {
                Ok(Some(frame)) => frame,
                Ok(None) => return Exit::Drained,
                Err(reason) => return Exit::Failed(reason),
            };
            let kernel = &mut self.storage.kernel;
            if !matches!(kernel.admit_remote_input(k::RemoteEndpointId(input.endpoint()),k::CordId(input.cord()),sequence,frame.canonical()),Ok(RemoteIngressOutcome::Accepted {sequence:accepted}) if accepted==sequence)
                || kernel
                    .close_remote_input(
                        k::RemoteEndpointId(input.endpoint()),
                        k::CordId(input.cord()),
                    )
                    .is_err()
            {
                return failed(k::FailureCode::InvalidLifecycle, 41);
            }
            fore.accepted(sequence);
            let mut drained = false;
            // Finite per-invocation work. Pressure or a stalled Back cannot grow
            // a trace indefinitely; refusal leaves the actual log in custody.
            for _ in 0..64 {
                match self.advance() {
                    Ok(SchedulerStatus::Drained) => {
                        drained = true;
                        break;
                    }
                    Ok(SchedulerStatus::Cancelled) => return Exit::Cancelled,
                    Err(exit) => return exit,
                    _ => (),
                }
                let offer = match self.storage.kernel.remote_egress_offer(
                    k::RemoteEndpointId(refusal.endpoint()),
                    k::CordId(refusal.cord()),
                ) {
                    Ok(offer) => offer,
                    Err(_) => return failed(k::FailureCode::InvalidLifecycle, 42),
                };
                if let Some(offer) = offer {
                    let valid = self
                        .storage
                        .kernel
                        .host_value(offer.value)
                        .ok()
                        .and_then(|bytes| {
                            conduit_core::validate_canonical_structured_value(bytes).ok()
                        })
                        .and_then(|value| value.primitive_bytes(conduit_core::EMPTY_INFO_ID).ok())
                        .is_some_and(|bytes| bytes.is_empty());
                    if !valid {
                        return failed(k::FailureCode::InvalidInput, 43);
                    }
                    if let Err(reason) = fore.refused(offer.sequence).await {
                        return Exit::Failed(reason);
                    }
                    if self
                        .storage
                        .kernel
                        .remote_egress_accept(offer.endpoint, offer.cord, offer.sequence)
                        .is_err()
                    {
                        return failed(k::FailureCode::InvalidLifecycle, 44);
                    }
                }
                yield_now().await;
            }
            if !drained {
                return failed(k::FailureCode::WorkBudgetExhausted, 45);
            }
            let page = match self.stage_evidence() {
                Ok(Some(page)) => page,
                _ => return failed(k::FailureCode::InvalidLifecycle, 46),
            };
            super::super::task::report_boundary(
                "evidence-pending",
                self.progress(),
                self.inspection(),
            );
            if super::super::evidence_task::wait(page).await.is_err() {
                return failed(k::FailureCode::InvalidLifecycle, 47);
            }
            super::super::task::report_boundary(
                "waiting-input",
                self.progress(),
                self.inspection(),
            );
            if sequence + 1 < u64::from(maximum_frames) && self.storage.kernel.begin_next().is_err()
            {
                return failed(k::FailureCode::InvalidLifecycle, 48);
            }
        }
        Exit::Drained
    }
}
fn failed(code: k::FailureCode, detail: u16) -> Exit {
    Exit::Failed(k::Failure { code, detail })
}
