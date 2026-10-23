//! Execute the target's exact dispatcher/assembly over inert CPU providers.
use super::*;
use conduit_environment_runtime::appliance::assigned::PreparedResidentSoftware;
use conduit_kernel::{self as k, scheduler::SchedulerStatus};
use core::{
    future::Future,
    task::{Context, Poll, Waker},
};
#[allow(clippy::too_many_arguments)]
pub(super) fn check<P: PulseReadProvider, W: w::MonotonicWaitProvider>(
    prepared: &conduit_environment_make::PreparedNativeAppliance,
    resident: &ResidentAppliance<'_>,
    required: c::AssignedPlanRequirements<'_>,
    mut native: native_authority::NativeBindings<P, W>,
    pulse_effects: &Cell<usize>,
    timer_effects: &Cell<usize>,
    active_wait: &Cell<bool>,
    behavior: u8,
) {
    let recipe = prepared.software_recipe().unwrap();
    let programs: Vec<_> = recipe.resident_computations().collect();
    let mut software = Some(Box::new(
        PreparedResidentSoftware::prepare(
            resident,
            &programs,
            &resident.cadence,
            conduit_environment_runtime::appliance::prepare_value_storage().unwrap(),
        )
        .unwrap(),
    ));
    let mut assignment = Some(Box::new(
        ValidatedAssignedAppliance::admit(
            prepared.assignment(),
            conduit_environment_make::NativeApplianceLimits::resident_local().assignment,
            required,
            resident,
        )
        .unwrap(),
    ));
    let transferred =
        PreparedResidentSoftware::bind_boxed_retained(&mut software, &mut assignment).unwrap();
    let lowered = prepared.lowered();
    let sign_slots = usize::from(lowered.sign_items)
        .max((lowered.sign_bytes as usize).div_ceil(core::mem::size_of::<k::KernelEvent>()));
    let mut storage = native_kernel_storage::assemble(
        transferred,
        k::HostedSignLog::new(sign_slots.try_into().unwrap(), lowered.sign_bytes).unwrap(),
    )
    .unwrap();
    if behavior == 4 {
        native
            .pulse
            .revoke(storage.roles.pulse, k::HostCallId(0))
            .unwrap();
    }
    let cancel = behavior == 1;
    let mut failure = None;
    let mut drained = false;
    let mut cancelled = false;
    if behavior == 6 {
        let ((), memory) = allocation_probe::observe(|| late_reply(&mut storage, &mut native));
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
        assert_eq!(pulse_effects.get(), 1);
        return;
    }
    if behavior == 5 {
        let ((), memory) = allocation_probe::observe(|| {
            concurrent_wait(
                &mut storage,
                &mut native,
                pulse_effects,
                timer_effects,
                active_wait,
            )
        });
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
        return;
    }
    let ((), memory) =
        allocation_probe::observe(|| {
            for _ in 0..resident.cadence.count * 2000 {
                let poll = poll_one(&mut storage, &mut native, active_wait);
                match poll {
                    Poll::Pending => {
                        assert!(cancel);
                        assert!(!active_wait.get());
                        native_service::quiesce(&mut storage, &mut native).unwrap();
                        assert_eq!(storage.kernel.step().unwrap(), SchedulerStatus::Cancelled);
                        cancelled = true;
                        break;
                    }
                    Poll::Ready(result) => {
                        if let Some(completed) = result.unwrap() {
                            if let Some(reason) = completed.failure {
                                assert_eq!(
                                    completed.request.node,
                                    if behavior == 4 {
                                        storage.roles.pulse
                                    } else {
                                        storage.roles.tick
                                    }
                                );
                                assert_eq!(
                                    completed.disposition,
                                    if behavior == 4 {
                                        k::HostCallDisposition::Denied
                                    } else {
                                        k::HostCallDisposition::Failed
                                    }
                                );
                                failure = Some(reason);
                                assert!(storage.kernel.signs().events().any(|event| event.kind
                                    == k::KernelEventKind::HostCallCompleted
                                    && event.node == completed.request.node));
                                native_service::quiesce(&mut storage, &mut native).unwrap();
                                break;
                            }
                        }
                    }
                }
                if storage.kernel.step().unwrap() == SchedulerStatus::Drained {
                    drained = true;
                    break;
                }
            }
        });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    if behavior >= 2 {
        assert!(!cancelled && !drained);
        assert_eq!(
            failure.unwrap().code,
            match behavior {
                2 => k::FailureCode::StorageExhausted,
                3 => k::FailureCode::HostCallFailed,
                _ => k::FailureCode::HostCallDenied,
            }
        );
        assert_eq!(pulse_effects.get(), 0);
        assert!(timer_effects.get() >= 1);
        if behavior != 4 {
            assert_eq!(timer_effects.get(), 1);
        }
        assert!(!active_wait.get());
        assert_eq!(
            native.wait.inspection().completed_operations as usize,
            timer_effects.get()
        );
    } else if cancel {
        assert!(failure.is_none());
        assert!(cancelled && !drained);
        assert_eq!((pulse_effects.get(), timer_effects.get()), (0, 1));
        for inspection in [native.pulse.inspection(), native.wait.inspection()] {
            assert_eq!(inspection.lifecycle, c::CapabilityLifecycle::Revoked);
            assert_eq!(inspection.in_flight_operations, 0);
        }
    } else {
        assert!(failure.is_none());
        assert!(drained && !cancelled);
        assert_eq!(
            (pulse_effects.get(), timer_effects.get()),
            (
                resident.cadence.count as usize,
                resident.cadence.count as usize
            )
        );
        for inspection in [native.pulse.inspection(), native.wait.inspection()] {
            assert_eq!(
                u64::from(inspection.completed_operations),
                resident.cadence.count
            );
            assert_eq!(inspection.in_flight_operations, 0);
        }
        let mut publications = 0;
        for driver in storage.kernel.drivers() {
            if let conduit_environment_runtime::appliance::ApplianceBack::Publication(back) = driver
            {
                if let Some(snapshot) = back.snapshot() {
                    assert_eq!(
                        snapshot.channel,
                        conduit_environment_runtime::publication::PublicationChannel::Reading
                    );
                    assert_eq!(snapshot.sequence, resident.cadence.count);
                    c::validate_canonical_structured_value(snapshot.canonical).unwrap();
                    publications += 1;
                }
            }
        }
        assert_eq!(publications, 1);
    }
}

fn poll_one<P: PulseReadProvider, W: w::MonotonicWaitProvider>(
    storage: &mut native_kernel_storage::PreparedKernel,
    native: &mut native_authority::NativeBindings<P, W>,
    active_wait: &Cell<bool>,
) -> Poll<Result<Option<native_service::ServicedCall>, k::Failure>> {
    use native_service::Dispatch;
    let reply = match native_service::next(storage).unwrap() {
        Dispatch::None => return Poll::Ready(Ok(None)),
        Dispatch::Software(call) => return Poll::Ready(Ok(Some(call))),
        Dispatch::Pulse(input) => {
            let mut invocation = core::pin::pin!(native_service::pulse(&mut native.pulse, input));
            invocation
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
        }
        Dispatch::Wait(input) => {
            let mut invocation = core::pin::pin!(native_service::wait(&mut native.wait, input));
            let polled = invocation
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()));
            if polled.is_pending() {
                assert!(active_wait.get());
            }
            polled
        }
    };
    match reply {
        Poll::Pending => Poll::Pending,
        Poll::Ready(reply) => Poll::Ready(native_service::finish(storage, reply).map(Some)),
    }
}
fn concurrent_wait<P: PulseReadProvider, W: w::MonotonicWaitProvider>(
    storage: &mut native_kernel_storage::PreparedKernel,
    native: &mut native_authority::NativeBindings<P, W>,
    pulse_effects: &Cell<usize>,
    timer_effects: &Cell<usize>,
    active_wait: &Cell<bool>,
) {
    use native_service::Dispatch;
    let mut published = false;
    for _ in 0..1000 {
        match native_service::next(storage).unwrap() {
            Dispatch::Wait(input) if timer_effects.get() == 1 => {
                {
                    let mut invocation =
                        core::pin::pin!(native_service::wait(&mut native.wait, input));
                    assert!(invocation
                        .as_mut()
                        .poll(&mut Context::from_waker(Waker::noop()))
                        .is_pending());
                    assert!(active_wait.get());
                    for _ in 0..1000 {
                        match native_service::next(storage).unwrap() {
                            Dispatch::None => (),
                            Dispatch::Software(call) => assert!(call.failure.is_none()),
                            Dispatch::Pulse(input) => {
                                let reply = run(native_service::pulse(&mut native.pulse, input));
                                assert!(native_service::finish(storage, reply)
                                    .unwrap()
                                    .failure
                                    .is_none());
                            }
                            Dispatch::Wait(_) => panic!("kernel duplicated its pending timer"),
                        }
                        storage.kernel.step().unwrap();
                        published = storage.kernel.drivers().iter().any(|driver| {
                            matches!(driver, conduit_environment_runtime::appliance::ApplianceBack::Publication(back)
                                if back.snapshot().is_some_and(|snapshot| snapshot.channel == conduit_environment_runtime::publication::PublicationChannel::Reading && snapshot.sequence == 1))
                        });
                        if published {
                            break;
                        }
                    }
                    assert!(published && active_wait.get());
                    assert_eq!((pulse_effects.get(), timer_effects.get()), (1, 2));
                }
                assert!(!active_wait.get());
                native_service::quiesce(storage, native).unwrap();
                assert_eq!(storage.kernel.step().unwrap(), SchedulerStatus::Cancelled);
                break;
            }
            Dispatch::Wait(input) => {
                let reply = run(native_service::wait(&mut native.wait, input));
                native_service::finish(storage, reply).unwrap();
            }
            Dispatch::Pulse(input) => {
                let reply = run(native_service::pulse(&mut native.pulse, input));
                native_service::finish(storage, reply).unwrap();
            }
            Dispatch::Software(call) => assert!(call.failure.is_none()),
            Dispatch::None => (),
        }
        storage.kernel.step().unwrap();
    }
    assert!(published);
}

fn late_reply<P: PulseReadProvider, W: w::MonotonicWaitProvider>(
    storage: &mut native_kernel_storage::PreparedKernel,
    native: &mut native_authority::NativeBindings<P, W>,
) {
    use k::ValueStorage;
    use native_service::Dispatch;
    let mut refused = false;
    for _ in 0..1000 {
        match native_service::next(storage).unwrap() {
            Dispatch::Pulse(input) => {
                let reply = run(native_service::pulse(&mut native.pulse, input));
                native_service::quiesce(storage, native).unwrap();
                let before = (
                    storage.kernel.values().used_items(),
                    storage.kernel.values().used_bytes(),
                );
                assert_eq!(
                    native_service::finish(storage, reply).unwrap_err().code,
                    k::FailureCode::InvalidLifecycle
                );
                assert_eq!(
                    (
                        storage.kernel.values().used_items(),
                        storage.kernel.values().used_bytes()
                    ),
                    before
                );
                assert_eq!(storage.kernel.step().unwrap(), SchedulerStatus::Cancelled);
                refused = true;
                break;
            }
            Dispatch::Wait(input) => {
                let reply = run(native_service::wait(&mut native.wait, input));
                native_service::finish(storage, reply).unwrap();
            }
            Dispatch::Software(call) => assert!(call.failure.is_none()),
            Dispatch::None => (),
        }
        storage.kernel.step().unwrap();
    }
    assert!(refused);
}
