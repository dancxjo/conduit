//! Synchronous observer composition; no network future ever borrows a kernel.
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
pub struct ForeObserver<'a, 'b, 'p, O> {
    existing: O,
    port: ObserverPort<'a, 'b, 'p>,
}
impl<'a, 'b, 'p, O> ForeObserver<'a, 'b, 'p, O> {
    pub fn new(existing: O, port: ObserverPort<'a, 'b, 'p>) -> Self {
        Self { existing, port }
    }
}
impl<
        D: StepBack<P>,
        L: SignSink,
        const N: usize,
        const C: usize,
        const P: usize,
        const R: usize,
        const T: usize,
        const H: usize,
        const K: usize,
        O: Observer<StoredExecutionKernel<D, L, N, C, P, R, T, H, K>>,
    > Observer<StoredExecutionKernel<D, L, N, C, P, R, T, H, K>> for ForeObserver<'_, '_, '_, O>
{
    type Error = ObserverRefusal<O::Error>;
    fn before_step(
        &mut self,
        kernel: &mut StoredExecutionKernel<D, L, N, C, P, R, T, H, K>,
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
        let input = Route::from_pin(state.ingress.pin);
        let output = Route::from_pin(state.egress.pin());
        if let Input::Offered { sequence, length } = state.input {
            match kernel.admit_remote_input(
                input.endpoint,
                input.cord,
                sequence,
                &state.ingress_bytes[..length],
            ) {
                Ok(RemoteIngressOutcome::Accepted { .. }) => {
                    state.input = Input::Accepted(sequence)
                }
                Ok(RemoteIngressOutcome::Full { .. }) => state.input = Input::Pressure(sequence),
                Err(error) if pressured(&error) => state.input = Input::Pressure(sequence),
                Err(error) => return Err(ObserverRefusal::Kernel(error)),
            }
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
                if let Err(error) =
                    kernel.remote_egress_delivered(output.endpoint, output.cord, sequence)
                {
                    if pressured(&error) {
                        return Ok(());
                    }
                    return Err(ObserverRefusal::Kernel(error));
                }
                state.output = Output::Empty;
            } else {
                state.output = Output::Offered {
                    sequence,
                    length,
                    phase: OutputPhase::Accepted,
                };
            }
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
        kernel: &mut StoredExecutionKernel<D, L, N, C, P, R, T, H, K>,
        dispatch: Dispatch,
    ) -> Result<(), Self::Error> {
        self.existing
            .completed(kernel, dispatch)
            .map_err(ObserverRefusal::Existing)
    }
}
