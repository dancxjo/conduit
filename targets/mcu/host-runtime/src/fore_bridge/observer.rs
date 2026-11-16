//! Synchronous observer composition; no network future ever borrows a kernel.
use super::archive_policy::{DictionaryEvidenceWindow, EvidencePolicy, NoArchive};
use super::*;
use crate::{execution::StoredExecutionKernel, native_dispatch::Dispatch};
use conduit_kernel::{
    scheduler::{RemoteIngressOutcome, SchedulerError, StepBack},
    SignSink, ValueStorage,
};
use mailbox::{Input, Output, OutputPhase};
fn pressured(error: &SchedulerError) -> bool {
    use conduit_kernel::{SignError as S, StorageError as V};
    matches!(
        error,
        SchedulerError::Sign(
            S::ItemCapacityExceeded
                | S::ByteCapacityExceeded
                | S::RemoteItemCapacityExceeded
                | S::RemoteByteCapacityExceeded
        ) | SchedulerError::Storage(V::ItemCapacityExceeded | V::ByteCapacityExceeded)
    )
}
#[derive(Debug)]
pub enum ObserverRefusal<E> {
    Existing(E),
    Kernel(SchedulerError),
    Bridge(Refusal),
}
pub struct ForeObserver<'a, 'b, 'p, O, A = NoArchive> {
    existing: O,
    port: ObserverPort<'a, 'b, 'p>,
    ingress_remaining: Option<u16>,
    archive: A,
}
impl<'a, 'b, 'p, O> ForeObserver<'a, 'b, 'p, O> {
    pub fn new(existing: O, port: ObserverPort<'a, 'b, 'p>) -> Self {
        Self {
            existing,
            port,
            ingress_remaining: None,
            archive: NoArchive,
        }
    }

    /// Native installation selects an explicitly funded finite ingress window.
    /// Exhaustion pressures the same item without touching the kernel or ACKing it.
    pub fn with_duplex_evidence_window(
        existing: O,
        port: ObserverPort<'a, 'b, 'p>,
        budget: crate::execution_sign_budget::DuplexSignBudget,
    ) -> Self {
        Self {
            existing,
            port,
            ingress_remaining: Some(budget.ingress_items()),
            archive: NoArchive,
        }
    }

    /// Force synchronous mailbox contention in cross-module actor tests.
    /// Tests must not perform IO while this loan is held.
    #[cfg(test)]
    pub(crate) fn with_test_mailbox_loan<R>(&self, apply: impl FnOnce() -> R) -> R {
        let _loan = self.port.shared.borrow_mut();
        apply()
    }
}
impl<'a, 'b, 'p, O> ForeObserver<'a, 'b, 'p, O, DictionaryEvidenceWindow> {
    /// Opt in only for a separately funded archive and a checked service that
    /// produces exactly one result per admitted input. This capacity law grants
    /// no native operations: all original Base and invocation limits still apply.
    pub fn with_archived_duplex_evidence_window(
        existing: O,
        port: ObserverPort<'a, 'b, 'p>,
        budget: crate::execution_sign_budget::DuplexSignBudget,
    ) -> Self {
        Self {
            existing,
            port,
            ingress_remaining: Some(budget.ingress_items()),
            archive: DictionaryEvidenceWindow::new(budget),
        }
    }
    /// Monotonic input, genuinely delivered result, and archived-result totals.
    pub fn evidence_window_totals(&self) -> (u64, u64, u64) {
        self.archive.totals()
    }
}
impl<
        D: StepBack<P>,
        L: SignSink,
        V: ValueStorage,
        const N: usize,
        const C: usize,
        const P: usize,
        const R: usize,
        const T: usize,
        const H: usize,
        const K: usize,
        const Q: usize,
        const M: usize,
        const F: usize,
        const U: usize,
        const Z: usize,
        O: Observer<StoredExecutionKernel<D, L, N, C, P, R, T, H, K, V, Q, M, F, U, Z>>,
        A: EvidencePolicy<StoredExecutionKernel<D, L, N, C, P, R, T, H, K, V, Q, M, F, U, Z>>,
    > Observer<StoredExecutionKernel<D, L, N, C, P, R, T, H, K, V, Q, M, F, U, Z>>
    for ForeObserver<'_, '_, '_, O, A>
{
    type Error = ObserverRefusal<O::Error>;
    fn before_step(
        &mut self,
        kernel: &mut StoredExecutionKernel<D, L, N, C, P, R, T, H, K, V, Q, M, F, U, Z>,
    ) -> Result<(), Self::Error> {
        self.existing
            .before_step(kernel)
            .map_err(ObserverRefusal::Existing)?;
        let Ok(mut state) = self.port.shared.try_borrow_mut() else {
            return Ok(());
        };
        if !state.active || self.port.retired.get() {
            return Ok(());
        }
        let input = Route::from_pin(state.ingress.pin());
        let output = Route::from_pin(state.egress.pin());
        macro_rules! process_ingress {
            () => {{
                if let Input::Offered { sequence, length } = state.input {
                    if self.ingress_remaining == Some(0) {
                        state.input = Input::Pressure(sequence);
                    } else {
                        if !self.archive.can_accept() {
                            return Err(ObserverRefusal::Bridge(Refusal::Bounds));
                        }
                        match kernel.admit_remote_input(
                            input.endpoint,
                            input.cord,
                            sequence,
                            &state.ingress_bytes[..length],
                        ) {
                            Ok(RemoteIngressOutcome::Accepted { .. }) => {
                                self.archive.accepted();
                                state.input = Input::Accepted(sequence);
                                if let Some(remaining) = &mut self.ingress_remaining {
                                    *remaining -= 1;
                                }
                            }
                            Ok(RemoteIngressOutcome::Full { .. }) => {
                                state.input = Input::Pressure(sequence)
                            }
                            Err(error) if pressured(&error) => {
                                state.input = Input::Pressure(sequence)
                            }
                            Err(error) => return Err(ObserverRefusal::Kernel(error)),
                        }
                    }
                }
            }};
        }
        if !A::RENEWING {
            process_ingress!();
        }
        if let Output::Offered {
            sequence,
            length,
            phase: OutputPhase::PeerAccepted | OutputPhase::PeerDelivered,
        } = state.output
        {
            // accept is idempotent, so a Delivered received before this pump can
            // apply both genuine peer acknowledgments in their original order.
            if let Err(error) = kernel.remote_egress_accept(output.endpoint, output.cord, sequence)
            {
                if pressured(&error) {
                    return Ok(());
                }
                return Err(ObserverRefusal::Kernel(error));
            }
            if matches!(
                state.output,
                Output::Offered {
                    phase: OutputPhase::PeerDelivered,
                    ..
                }
            ) {
                if !self.archive.can_deliver() {
                    return Err(ObserverRefusal::Bridge(Refusal::Bounds));
                }
                if let Err(error) =
                    kernel.remote_egress_delivered(output.endpoint, output.cord, sequence)
                {
                    if pressured(&error) {
                        return Ok(());
                    }
                    return Err(ObserverRefusal::Kernel(error));
                }
                self.archive.delivered();
                state.output = Output::Empty;
            } else {
                state.output = Output::Offered {
                    sequence,
                    length,
                    phase: OutputPhase::Accepted,
                };
            }
        }
        // Genuine delivery and successful same-owner transfer precede the next
        // admission, including when both mailbox directions arrive together.
        self.archive.renew(kernel, &mut self.ingress_remaining);
        if A::RENEWING {
            process_ingress!();
        }
        if matches!(state.output, Output::Empty) {
            let offer = match kernel.remote_egress_offer(output.endpoint, output.cord) {
                Ok(offer) => offer,
                Err(error) if pressured(&error) => return Ok(()),
                Err(error) => return Err(ObserverRefusal::Kernel(error)),
            };
            if let Some(offer) = offer {
                let bytes = kernel
                    .values()
                    .get(offer.value)
                    .map_err(|e| ObserverRefusal::Kernel(SchedulerError::Storage(e)))?;
                if bytes.len() > state.egress.byte_capacity() as usize {
                    return Err(ObserverRefusal::Bridge(Refusal::Bounds));
                }
                state.egress_bytes[..bytes.len()].copy_from_slice(bytes);
                state.output = Output::Offered {
                    sequence: offer.sequence,
                    length: bytes.len(),
                    phase: OutputPhase::Ready,
                };
            }
        }
        Ok(())
    }
    fn completed(
        &mut self,
        kernel: &mut StoredExecutionKernel<D, L, N, C, P, R, T, H, K, V, Q, M, F, U, Z>,
        dispatch: Dispatch,
    ) -> Result<(), Self::Error> {
        self.existing
            .completed(kernel, dispatch)
            .map_err(ObserverRefusal::Existing)
    }
}
