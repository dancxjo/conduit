//! Native bus/deadline actors around the same Started kernel and scheduler.
use super::*;
use conduit_i2c_transaction::{owner::I2cHostCall, I2cProvider};
use conduit_monotonic_wait::clock_at::owner::MonotonicDeadlineProvider;
use native_dispatch::deadline;

impl<
        D: StepBack<P>,
        const N: usize,
        const C: usize,
        const P: usize,
        const R: usize,
        const T: usize,
        const H: usize,
        Y: CooperativeYield,
        O: Observer<StoredExecutionKernel<D, L, N, C, P, R, T, H, K, V>>,
        F: Fn() -> bool,
        L: SignSink,
        const K: usize,
        V: ValueStorage,
    > Runner<'_, D, N, C, P, R, T, H, Y, O, F, L, K, V>
{
    async fn deadline<A: MonotonicDeadlineProvider>(
        &self,
        service: &mut deadline::Service<'_, A>,
    ) -> Result<(), Refusal<O::Error>> {
        while !self.done.get() {
            if let Ok(mut kernel) = self.kernel.try_borrow_mut() {
                let dispatch = service.dispatch(&mut **kernel).map_err(Refusal::Kernel)?;
                if matches!(dispatch, Dispatch::Completed { .. }) {
                    self.observer
                        .borrow_mut()
                        .completed(&mut **kernel, dispatch)
                        .map_err(Refusal::Observer)?;
                }
            }
            self.yield_to_host.pause().await;
        }
        Ok(())
    }
    /// Services only the original independently bound owners after lawful Start.
    /// Bus access shares the same physical timing exclusion as other fragments;
    /// deadline polling stays cooperative and never holds a kernel loan over a
    /// wait. No Source preparation, grant, retry or transport policy occurs here.
    pub async fn run_with_i2c_clock<I: I2cProvider, A: MonotonicDeadlineProvider>(
        self,
        bus: &mut synchronous::Service<'_, I2cHostCall<I>>,
        clock: &mut deadline::Service<'_, A>,
    ) -> Result<Exit, Refusal<O::Error>> {
        let validation = if self.activity.is_none() {
            Err(synchronous::Refusal::MissingActivity)
        } else if bus.node() == clock.node() {
            Err(synchronous::Refusal::ConflictingNode)
        } else {
            Ok(())
        };
        let result = match validation {
            Err(error) => Err(Refusal::Synchronous(error)),
            Ok(()) => {
                crate::native_supervisor::supervise(
                    self.scheduler(),
                    self.synchronous(Some(bus)),
                    self.deadline(clock),
                    async { Ok(()) },
                )
                .await
            }
        };
        // Every actor future is dropped by supervise before either owner is
        // revoked or kernel storage is cancelled. Other fragments keep playing.
        let bus_retirement = if result.is_ok() {
            bus.finish()
        } else {
            bus.revoke()
        };
        let bus_revoked = bus_retirement.is_ok();
        if !bus_revoked {
            let _ = bus.revoke();
        }
        let clock_revoked = clock.revoke().is_ok();
        let kernel = self.kernel.into_inner();
        let cancellation = if result.is_err() {
            kernel.cancel().err()
        } else {
            None
        };
        if !bus_revoked || !clock_revoked || cancellation.is_some() {
            return Err(Refusal::Retirement {
                kernel: cancellation,
                native_revocation_failed: !bus_revoked || !clock_revoked,
            });
        }
        result.map(|()| Exit::Drained)
    }
}
