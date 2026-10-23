//! CPU fixture executor for the exact target worker, not physical clock proof.
use super::*;
use crate::native_worker::{Execution, Exit};
use conduit_environment_runtime::publication::{PublicationChannel, PublicationSnapshot};
#[allow(clippy::too_many_arguments)]
pub(super) fn check<P: PulseReadProvider, W: w::MonotonicWaitProvider>(
    storage: native_kernel_storage::PreparedKernel,
    native: native_authority::NativeBindings<P, W>,
    pulse_effects: &Cell<usize>,
    timer_effects: &Cell<usize>,
    active_wait: &Cell<bool>,
    samples: u64,
    behavior: u8,
    retirement: impl FnOnce(crate::native_worker::RetiredProviders<P, W>),
) {
    let mut storage = Some(storage);
    let mut native = Some(native);
    let original_kernel = (&*storage.as_ref().unwrap().kernel) as *const _;
    let held_native = native.take();
    let (refused, memory) =
        allocation_probe::observe(|| Execution::boxed_retained(&mut storage, &mut native));
    assert!(refused.is_err() && storage.is_some());
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    native = held_native;
    let held_storage = storage.take();
    let (refused, memory) =
        allocation_probe::observe(|| Execution::boxed_retained(&mut storage, &mut native));
    assert!(refused.is_err() && native.is_some());
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    storage = held_storage;
    let (result, memory) =
        allocation_probe::observe(|| Execution::boxed_retained(&mut storage, &mut native));
    let mut execution = result.unwrap();
    assert!(storage.is_none() && native.is_none());
    assert_eq!((memory.allocations, memory.reallocations), (1, 0));
    assert_eq!(memory.requested_bytes, core::mem::size_of_val(&*execution));
    assert_eq!((&*execution.storage.kernel) as *const _, original_kernel);
    let mut retained = Some(execution);
    let (refused, memory) = allocation_probe::observe(|| Execution::retire_retained(&mut retained));
    assert_eq!(refused.err(), Some("execution-cleanup-unconfirmed"));
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    execution = retained.take().unwrap();
    assert_eq!((&*execution.storage.kernel) as *const _, original_kernel);
    if behavior == 14 {
        let (cancelled, memory) = allocation_probe::observe(|| execution.cancel_held());
        let cancelled = cancelled.unwrap();
        assert_eq!(cancelled.exit, Exit::Cancelled);
        assert_eq!(cancelled.cleanup, Ok(()));
        assert_eq!(cancelled.progress, Default::default());
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
        assert_eq!((pulse_effects.get(), timer_effects.get()), (0, 0));
        retirement(retire(execution));
        return;
    }

    if behavior == 12 {
        execution
            .native
            .pulse
            .revoke(execution.storage.roles.pulse, k::HostCallId(0))
            .unwrap();
    }
    let publication = Cell::new(0_u64);
    let stop = || {
        core::future::poll_fn(|_| {
            if (behavior == 8 && active_wait.get())
                || (behavior == 9 && active_wait.get() && publication.get() == 1)
            {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        })
    };
    let mut observer = |snapshot: PublicationSnapshot<'_>| {
        assert_eq!(snapshot.channel, PublicationChannel::Reading);
        publication.set(snapshot.sequence);
    };
    let bytes = execution.future_bytes(stop(), &mut observer);
    assert!(bytes > 0);
    assert_eq!((pulse_effects.get(), timer_effects.get()), (0, 0));
    assert!(!active_wait.get());
    if behavior == 13 {
        let ((), memory) = allocation_probe::observe(|| {
            {
                let mut future =
                    core::pin::pin!(execution.run(core::future::pending(), &mut observer));
                for _ in 0..1000 {
                    assert!(future
                        .as_mut()
                        .poll(&mut Context::from_waker(Waker::noop()))
                        .is_pending());
                    if active_wait.get() {
                        break;
                    }
                }
                assert!(active_wait.get());
            }
            assert!(!active_wait.get());
            let fenced = run(execution.run(core::future::ready(()), &mut observer));
            assert_eq!(
                fenced.exit,
                Exit::Failed(k::Failure {
                    code: k::FailureCode::InvalidLifecycle,
                    detail: 27
                })
            );
            assert_eq!(fenced.cleanup, Ok(()));
            for inspection in [
                execution.native.pulse.inspection(),
                execution.native.wait.inspection(),
            ] {
                assert_eq!(inspection.lifecycle, c::CapabilityLifecycle::Revoked);
                assert_eq!(inspection.in_flight_operations, 0);
            }
            assert_eq!(
                execution.storage.kernel.step().unwrap(),
                SchedulerStatus::Cancelled
            );
        });
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
        assert_eq!((pulse_effects.get(), timer_effects.get()), (0, 1));
        retirement(retire(execution));
        return;
    }
    let (outcome, memory) = allocation_probe::observe(|| {
        let mut future = core::pin::pin!(execution.run(stop(), &mut observer));
        for _ in 0..samples * 2000 {
            if let Poll::Ready(outcome) = future
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
            {
                return outcome;
            }
        }
        panic!("bounded CPU worker did not terminate");
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert_eq!(outcome.cleanup, Ok(()));
    assert!(!active_wait.get());
    for inspection in [
        execution.native.pulse.inspection(),
        execution.native.wait.inspection(),
    ] {
        assert_eq!(inspection.lifecycle, c::CapabilityLifecycle::Revoked);
        assert_eq!(inspection.in_flight_operations, 0);
    }
    match behavior {
        7 => {
            assert_eq!(outcome.exit, Exit::Drained);
            assert_eq!(outcome.progress.submitted, [samples; 2]);
            assert_eq!(outcome.progress.completed, [samples; 2]);
            assert_eq!(outcome.progress.publications, samples);
            assert_eq!(publication.get(), samples);
            assert_eq!(
                (pulse_effects.get(), timer_effects.get()),
                (samples as usize, samples as usize)
            );
            assert_eq!(
                execution.storage.kernel.step().unwrap(),
                SchedulerStatus::Drained
            );
            eprintln!("native_worker_cpu pointer_bits={} context_bytes={} future_bytes={bytes} samples={samples} queue_capacities=1,1,2 play_allocations=0",usize::BITS,core::mem::size_of_val(&*execution));
        }
        8 => {
            assert_eq!(outcome.exit, Exit::Cancelled);
            assert_eq!((pulse_effects.get(), timer_effects.get()), (0, 1));
        }
        9 => {
            assert_eq!(outcome.exit, Exit::Cancelled);
            assert_eq!(publication.get(), 1);
            assert_eq!((pulse_effects.get(), timer_effects.get()), (1, 2));
        }
        10..=12 => {
            let Exit::Failed(reason) = outcome.exit else {
                panic!("worker hid native failure");
            };
            assert_eq!(
                reason.code,
                match behavior {
                    10 => k::FailureCode::StorageExhausted,
                    11 => k::FailureCode::HostCallFailed,
                    _ => k::FailureCode::HostCallDenied,
                }
            );
            assert_eq!(pulse_effects.get(), 0);
            assert!(timer_effects.get() >= 1);
            assert_eq!(publication.get(), 0);
        }
        _ => unreachable!(),
    }
    let replay = run(execution.run(core::future::ready(()), |_| {}));
    assert!(matches!(
        replay.exit,
        Exit::Failed(k::Failure {
            code: k::FailureCode::InvalidLifecycle,
            ..
        })
    ));
    assert_eq!(replay.progress, outcome.progress);
    assert_eq!(
        execution.cancel_held().err(),
        Some("execution-not-unpolled")
    );
    retirement(retire(execution));
}
fn retire<P: PulseReadProvider, W: w::MonotonicWaitProvider>(
    execution: Box<Execution<P, W>>,
) -> crate::native_worker::RetiredProviders<P, W> {
    let mut retained = Some(execution);
    let (providers, memory) =
        allocation_probe::observe(|| Execution::retire_retained(&mut retained));
    let providers = providers.unwrap();
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert!(providers.pulse.1.generation > 0 && providers.wait.1.generation > 0);
    assert!(retained.is_none());
    assert_eq!(
        Execution::retire_retained(&mut retained).err(),
        Some("execution-unavailable")
    );
    providers
}
