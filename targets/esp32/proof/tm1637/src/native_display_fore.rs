//! Owned local Fore mechanics. Root separately admits the actual producer,
//! consumer, scope, finite work and complete future backing before Release.
use super::{Execution, Exit, Outcome};
use conduit_environment_runtime::display_packets::validate_display_frame;
use conduit_kernel::{
    self as k,
    scheduler::{RemoteIngressOutcome, SchedulerStatus},
};
use conduit_tm1637_wire::OpenDrain;
use core::future::Future;
use embassy_futures::{
    select::{select, Either},
    yield_now,
};

/// A finite owned value, not a caller permit, Line packet or hardware command.
pub struct Frame {
    bytes: [u8; 1024],
    length: u16,
}
impl Frame {
    pub fn copy(canonical: &[u8]) -> Result<Self, conduit_core::StructuredInfoRefusal> {
        validate_display_frame(canonical)?;
        let mut frame = Self {
            bytes: [0; 1024],
            length: canonical.len() as u16,
        };
        frame.bytes[..canonical.len()].copy_from_slice(canonical);
        Ok(frame)
    }
    pub fn canonical(&self) -> &[u8] {
        &self.bytes[..usize::from(self.length)]
    }
}

/// Exclusive process-local adapter over the exact prepared Fore. Implementing
/// this trait establishes no authority or Body membership. Root must select and
/// retain the real caller and consumer; no transport may impersonate this owner.
/// Futures must use separately admitted backing and work, with no hidden queues.
pub trait LocalFore {
    /// None is normal close. Pending is pressure/no input, never an empty Frame.
    fn next(&mut self) -> impl Future<Output = Result<Option<Frame>, k::Failure>>;
    /// Kernel acceptance is distinct from electrical completion or delivery.
    fn accepted(&mut self, sequence: u64);
    /// Acknowledge the exact refusal output item only after its consumer accepts.
    /// This sequence belongs to the refusal Fore, not the original input Frame.
    fn refused(&mut self, sequence: u64) -> impl Future<Output = Result<(), k::Failure>>;
}
impl<P: OpenDrain> Execution<P> {
    /// The caller exclusively owns both Fore directions. Pull only at an idle
    /// boundary, keeping one Frame in flight. The explicit input bound includes
    /// semantic refusals and may not exceed the retained native write budget.
    /// This entrance grants nothing and is not connected to Root control/UART.
    pub async fn run_with_fore(
        &mut self,
        stop: impl Future<Output = ()>,
        fore: &mut impl LocalFore,
        maximum_frames: u32,
    ) -> Outcome {
        if let Some(outcome) = self.begin_run() {
            return outcome;
        }
        if self.storage.kernel.decisions() != 0
            || self.storage.kernel.logical_invocations() != 0
            || self.storage.kernel.pending_host_call_count() != 0
            || k::ValueStorage::used_items(self.storage.kernel.values()) != 0
            || !k::SignSink::is_empty(self.storage.kernel.signs())
        {
            return self.finish_run(failed(k::FailureCode::InvalidLifecycle, 19));
        }
        if maximum_frames == 0 || maximum_frames > self.inspection().scope.maximum_operations {
            return self.finish_run(failed(k::FailureCode::WorkBudgetExhausted, 20));
        }
        let exit = match select(stop, self.pump_fore(fore, maximum_frames)).await {
            Either::First(()) => Exit::Cancelled,
            Either::Second(exit) => exit,
        };
        self.finish_run(exit)
    }
    async fn pump_fore(&mut self, fore: &mut impl LocalFore, maximum_frames: u32) -> Exit {
        let input = self.storage.input;
        let refusal = self.storage.refusal;
        let mut sequence = 0u64;
        let mut closed = false;
        loop {
            let offer = match self.storage.kernel.remote_egress_offer(
                k::RemoteEndpointId(refusal.endpoint()),
                k::CordId(refusal.cord()),
            ) {
                Ok(offer) => offer,
                Err(_) => return failed(k::FailureCode::InvalidPort, 21),
            };
            if let Some(offer) = offer {
                let valid = self
                    .storage
                    .kernel
                    .host_value(offer.value)
                    .ok()
                    .and_then(|bytes| conduit_core::validate_canonical_structured_value(bytes).ok())
                    .and_then(|value| value.primitive_bytes(conduit_core::EMPTY_INFO_ID).ok())
                    .is_some_and(|bytes| bytes.is_empty());
                if !valid {
                    return failed(k::FailureCode::InvalidInput, 22);
                }
                if let Err(reason) = fore.refused(offer.sequence).await {
                    return Exit::Failed(reason);
                }
                if self
                    .storage
                    .kernel
                    .remote_egress_accept(offer.endpoint, offer.cord, offer.sequence)
                    .and_then(|()| {
                        self.storage.kernel.remote_egress_delivered(
                            offer.endpoint,
                            offer.cord,
                            offer.sequence,
                        )
                    })
                    .is_err()
                {
                    return failed(k::FailureCode::InvalidLifecycle, 23);
                }
            }
            match self.advance() {
                Ok(SchedulerStatus::Drained) => return Exit::Drained,
                Ok(SchedulerStatus::Cancelled) => return Exit::Cancelled,
                Err(exit) => return exit,
                Ok(SchedulerStatus::Idle) => {
                    if closed {
                        return failed(k::FailureCode::InvalidLifecycle, 24);
                    }
                    match fore.next().await {
                        Err(reason) => return Exit::Failed(reason),
                        Ok(None) => {
                            if self
                                .storage
                                .kernel
                                .close_remote_input(
                                    k::RemoteEndpointId(input.endpoint()),
                                    k::CordId(input.cord()),
                                )
                                .is_err()
                            {
                                return failed(k::FailureCode::InvalidLifecycle, 25);
                            }
                            closed = true;
                        }
                        Ok(Some(frame)) => {
                            if sequence >= u64::from(maximum_frames) {
                                return failed(k::FailureCode::WorkBudgetExhausted, 26);
                            }
                            match self.storage.kernel.admit_remote_input(
                                k::RemoteEndpointId(input.endpoint()),
                                k::CordId(input.cord()),
                                sequence,
                                frame.canonical(),
                            ) {
                                Ok(RemoteIngressOutcome::Accepted { sequence: accepted })
                                    if accepted == sequence =>
                                {
                                    fore.accepted(sequence);
                                    sequence += 1;
                                }
                                // Pull-at-idle admits one outstanding item. A full
                                // input here violates exclusive local ownership;
                                // do not overwrite, drop-and-retry or spin it.
                                _ => return failed(k::FailureCode::InvalidLifecycle, 27),
                            }
                        }
                    }
                }
                _ => yield_now().await,
            }
        }
    }
}
fn failed(code: k::FailureCode, detail: u16) -> Exit {
    Exit::Failed(k::Failure { code, detail })
}
