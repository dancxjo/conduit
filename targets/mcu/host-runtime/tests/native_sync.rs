//! Actual CPU kernel/opaque-native fixture; not Root admission/physical proof.
extern crate alloc;
use conduit_kernel::{scheduler::*, *};
use conduit_mcu_host_runtime::{
    execution::StoredExecutionKernel,
    native_dispatch::Dispatch,
    native_runner::{synchronous::*, NativeActivity},
};
use conduit_resource_snapshot::native_append::{AppendOutcome, APPEND_RESULT_BYTES};
use std::{cell::Cell, rc::Rc};
#[path = "../../../../mechanisms/io/resource-snapshot/tests/support/native_append.rs"]
mod fixture;
struct RequestBack {
    input: ValueRef,
    phase: u8,
}
impl StepBack<1> for RequestBack {
    fn step(&mut self, io: &mut StepIo<1>, bytes: &StepInputBytes<'_, 1>) -> StepOutcome {
        if self.phase == 0 {
            io.request_host_call(
                RequestId(0),
                HostCallId(0),
                BoundedValueRef::new(self.input, 200).unwrap(),
            )
            .unwrap();
            self.phase = 1;
            return StepOutcome::Progress;
        }
        if let Some((request, outcome)) = io.host_completion() {
            assert_eq!(request, RequestId(0));
            assert_eq!(outcome.disposition, HostCallDisposition::Completed);
            assert_eq!(
                conduit_core::validate_canonical_structured_value(bytes.host_output().unwrap())
                    .unwrap()
                    .variant_tag()
                    .unwrap(),
                "recorded"
            );
            io.consume_host_completion().unwrap();
            self.phase = 2;
            StepOutcome::Complete
        } else {
            StepOutcome::Await
        }
    }
}
struct Signs {
    log: FixedSignLog<64>,
    fail: Rc<Cell<bool>>,
}
impl SignSink for Signs {
    fn len(&self) -> u16 {
        self.log.len()
    }
    fn item_capacity(&self) -> u16 {
        self.log.item_capacity()
    }
    fn byte_capacity(&self) -> u32 {
        self.log.byte_capacity()
    }
    fn used_bytes(&self) -> u32 {
        self.log.used_bytes()
    }
    fn ensure_capacity(&self, n: u16) -> Result<(), SignError> {
        if self.fail.get() {
            Err(SignError::ItemCapacityExceeded)
        } else {
            self.log.ensure_capacity(n)
        }
    }
    fn record(
        &mut self,
        n: NodeId,
        p: Option<PortId>,
        r: Option<RequestId>,
        k: KernelEventKind,
    ) -> Result<KernelEvent, SignError> {
        self.log.record(n, p, r, k)
    }
    fn record_remote(
        &mut self,
        n: NodeId,
        p: PortId,
        k: KernelEventKind,
        r: RemoteLifecycleIdentity,
    ) -> Result<KernelEvent, SignError> {
        self.log.record_remote(n, p, k, r)
    }
    fn ensure_remote_capacity(&self, n: u16) -> Result<(), SignError> {
        self.log.ensure_remote_capacity(n)
    }
}
type Kernel = StoredExecutionKernel<RequestBack, Signs, 1, 1, 1, 0, 0, 1>;
fn kernel(value_bytes: u32, fail: Rc<Cell<bool>>) -> Kernel {
    let mut values = HostedValueStore::new(4, 200, value_bytes).unwrap();
    let input = values.store(b"sample").unwrap();
    let mut routes = FixedRoutes::<0, 0>::new(1);
    routes.seal().unwrap();
    let mut bindings = FixedHostCallBindings::<1>::new(1);
    bindings
        .install(
            NodeId(0),
            HostCallBinding {
                call: HostCallId(0),
                maximum_input_bytes: 200,
                maximum_output_bytes: APPEND_RESULT_BYTES,
            },
        )
        .unwrap();
    bindings.seal().unwrap();
    Kernel::new_with_active_counts_and_host_calls(
        1,
        0,
        [NodeSpec {
            input_cords: [None],
            maximum_step_fuel: 8,
        }],
        [CordSpec::local(
            CordId(0),
            (NodeId(0), PortId(0)),
            (NodeId(0), PortId(0)),
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: 200,
                pressure_policy: Default::default(),
            },
        )],
        routes,
        bindings,
        [RequestBack { input, phase: 0 }],
        values,
        Signs {
            log: FixedSignLog::new(64 * core::mem::size_of::<KernelEvent>() as u32).unwrap(),
            fail,
        },
    )
    .unwrap()
}
#[test]
fn real_kernel_accepts_exact_media_result_then_acknowledges_original_owner() {
    let f = fixture::Fixture::new();
    let mut owner = f.bind(false).unwrap();
    let activity = NativeActivity::new();
    let mut kernel = kernel(800, Rc::new(Cell::new(false)));
    kernel.step().unwrap();
    let mut output = [0; APPEND_RESULT_BYTES as usize];
    let mut service = Service::new(&mut owner, &mut output).unwrap();
    let timing = activity.try_timing().unwrap();
    assert_eq!(service.dispatch(&mut kernel, &activity), Ok(Dispatch::Idle));
    assert_eq!(f.storage.programs.get(), 0);
    drop(timing);
    assert!(matches!(
        service.dispatch(&mut kernel, &activity),
        Ok(Dispatch::Completed { .. })
    ));
    assert!(service.bound_owner().pending_outcome().unwrap().2);
    assert!(service.bound_owner().custody().unwrap().kernel_committed);
    assert_eq!(f.storage.programs.get(), 2);
    assert_eq!(service.dispatch(&mut kernel, &activity), Ok(Dispatch::Idle));
    kernel.step().unwrap();
    service.revoke().unwrap();
    assert_eq!(f.storage.programs.get(), 2);
}
#[test]
fn real_kernel_pressure_and_sign_pressure_leave_request_before_effects() {
    for storage_pressure in [false, true] {
        let f = fixture::Fixture::new();
        let mut owner = f.bind(false).unwrap();
        let activity = NativeActivity::new();
        let fail = Rc::new(Cell::new(false));
        let mut kernel = kernel(if storage_pressure { 191 } else { 800 }, fail.clone());
        kernel.step().unwrap();
        if !storage_pressure {
            fail.set(true);
        }
        let mut output = [0; APPEND_RESULT_BYTES as usize];
        let mut service = Service::new(&mut owner, &mut output).unwrap();
        assert_eq!(
            service.dispatch(&mut kernel, &activity),
            Ok(Dispatch::Pressure)
        );
        assert_eq!(f.storage.programs.get(), 0);
        assert!(service.bound_owner().pending_outcome().is_none());
        if !storage_pressure {
            fail.set(false);
            assert!(matches!(
                service.dispatch(&mut kernel, &activity),
                Ok(Dispatch::Completed { .. })
            ));
        }
        service.revoke().unwrap();
    }
}
struct Interrupt<P> {
    inner: P,
    fail: Rc<Cell<bool>>,
}
// SAFETY: delegates actual native custody; injects only post-invoke CPU Sign refusal.
unsafe impl<P: SynchronousHostCall> SynchronousHostCall for Interrupt<P> {
    fn node(&self) -> NodeId {
        self.inner.node()
    }
    fn call(&self) -> HostCallId {
        self.inner.call()
    }
    fn maximum_input_bytes(&self) -> u32 {
        self.inner.maximum_input_bytes()
    }
    fn maximum_output_bytes(&self) -> u32 {
        self.inner.maximum_output_bytes()
    }
    fn invoke(&mut self, r: HostCallRequest, i: &[u8], o: &mut [u8]) -> Result<usize, CallFailure> {
        let result = self.inner.invoke(r, i, o);
        self.fail.set(true);
        result
    }
    fn acknowledge(&mut self, r: HostCallRequest) -> Result<(), CallFailure> {
        self.inner.acknowledge(r)
    }
    fn revoke(&mut self) -> Result<(), CallFailure> {
        self.inner.revoke()
    }
}
#[test]
fn rejected_kernel_completion_never_acknowledges_or_repeats_committed_media() {
    let f = fixture::Fixture::new();
    let fail = Rc::new(Cell::new(false));
    let mut owner = Interrupt {
        inner: f.bind(false).unwrap(),
        fail: fail.clone(),
    };
    let mut kernel = kernel(800, fail.clone());
    kernel.step().unwrap();
    let activity = NativeActivity::new();
    let mut output = [0; APPEND_RESULT_BYTES as usize];
    let mut service = Service::new(&mut owner, &mut output).unwrap();
    assert!(matches!(
        service.dispatch(&mut kernel, &activity),
        Err(Refusal::Kernel(SchedulerError::Sign(_)))
    ));
    assert_eq!(f.storage.programs.get(), 2);
    assert_eq!(
        service.bound_owner().inner.pending_outcome().unwrap().1,
        AppendOutcome::Recorded(conduit_resource_snapshot::append::DurableRecordIndex(0))
    );
    assert!(
        !service
            .bound_owner()
            .inner
            .custody()
            .unwrap()
            .kernel_committed
    );
    assert_eq!(service.dispatch(&mut kernel, &activity), Ok(Dispatch::Idle));
    assert_eq!(f.storage.programs.get(), 2);
    service.revoke().unwrap();
    fail.set(false);
    kernel.cancel().unwrap();
    assert!(
        !service
            .bound_owner()
            .inner
            .custody()
            .unwrap()
            .kernel_committed
    );
}

struct NoHardware;
impl conduit_pulse_capture::owner::PulseReadProvider for NoHardware {
    async fn read(
        &mut self,
        _: conduit_pulse_capture::CheckedPulseRead,
    ) -> Result<conduit_pulse_capture::PulseReadCapture, conduit_pulse_capture::PulseReadRefusal>
    {
        panic!("no installed pulse owner")
    }
}
impl conduit_tm1637_wire::OpenDrain for NoHardware {
    type Error = ();
    fn clock(&mut self, _: bool) -> Result<(), ()> {
        panic!("no installed write owner")
    }
    fn data(&mut self, _: bool) -> Result<(), ()> {
        panic!("no installed write owner")
    }
    fn read_data(&mut self) -> Result<bool, ()> {
        panic!("no installed write owner")
    }
    fn half_period(&mut self) -> Result<(), ()> {
        panic!("no installed write owner")
    }
}
impl conduit_monotonic_wait::MonotonicWaitProvider for NoHardware {
    async fn wait(&mut self, _: u64) -> Result<(), conduit_monotonic_wait::WaitProviderFailure> {
        panic!("no installed wait owner")
    }
}
struct Yield;
impl conduit_mcu_host_runtime::native_runner::CooperativeYield for Yield {
    async fn pause(&self) {
        let mut first = true;
        core::future::poll_fn(move |cx| {
            if first {
                first = false;
                cx.waker().wake_by_ref();
                core::task::Poll::Pending
            } else {
                core::task::Poll::Ready(())
            }
        })
        .await
    }
}
#[derive(Default)]
struct Observer {
    completed: usize,
}
impl conduit_mcu_host_runtime::native_runner::Observer<Kernel> for Observer {
    type Error = &'static str;
    fn before_step(&mut self, _: &mut Kernel) -> Result<(), Self::Error> {
        Ok(())
    }
    fn completed(&mut self, kernel: &mut Kernel, dispatch: Dispatch) -> Result<(), Self::Error> {
        let Dispatch::Completed { outcome, .. } = dispatch else {
            return Err("not completed");
        };
        let bytes = kernel
            .host_value(outcome.output.ok_or("no output")?.value)
            .map_err(|_| "not owned")?;
        assert_eq!(
            conduit_core::validate_canonical_structured_value(bytes)
                .unwrap()
                .variant_tag()
                .unwrap(),
            "recorded"
        );
        self.completed += 1;
        Ok(())
    }
}
fn finish<F: core::future::Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut cx = core::task::Context::from_waker(core::task::Waker::noop());
    for _ in 0..64 {
        if let core::task::Poll::Ready(value) = future.as_mut().poll(&mut cx) {
            return value;
        }
    }
    panic!("bounded CPU runner did not finish")
}
#[test]
fn public_runner_services_original_kernel_and_retires_on_drain_missing_window_or_cancel() {
    use conduit_mcu_host_runtime::{
        execution_factory::Calls,
        native_runner::{Exit, Refusal as RunRefusal, Runner},
    };
    for mode in 0..3 {
        let f = fixture::Fixture::new();
        let mut owner = f.bind(false).unwrap();
        let mut kernel = kernel(800, Rc::new(Cell::new(false)));
        let activity = NativeActivity::new();
        let mut output = [0; APPEND_RESULT_BYTES as usize];
        let mut service = Service::new(&mut owner, &mut output).unwrap();
        let mut calls = Calls::<NoHardware, NoHardware, NoHardware, 0> {
            pulse: None,
            write: None,
            waits: [],
        };
        let mut observer = Observer::default();
        let yield_to_host = Yield;
        let cancelled = || mode == 2;
        let runner = Runner::new(&mut kernel, &mut observer, &yield_to_host, &cancelled);
        let runner = if mode == 1 {
            runner
        } else {
            runner.with_activity(&activity)
        };
        let result = finish(runner.run_with_sync_calls(&mut calls, &mut service));
        match mode {
            0 => {
                assert!(matches!(result, Ok(Exit::Drained)));
                assert_eq!(observer.completed, 1);
                assert!(service.bound_owner().custody().unwrap().kernel_committed);
                assert_eq!(f.storage.programs.get(), 2);
            }
            1 => {
                assert!(matches!(
                    result,
                    Err(RunRefusal::Synchronous(Refusal::MissingActivity))
                ));
                assert_eq!(observer.completed, 0);
                assert_eq!(f.storage.programs.get(), 0);
            }
            _ => {
                assert!(matches!(
                    result,
                    Err(RunRefusal::Kernel(SchedulerError::Cancelled))
                ));
                assert_eq!(observer.completed, 0);
                assert_eq!(f.storage.programs.get(), 0);
            }
        }
        assert_eq!(
            service.bound_owner().inspection().lifecycle,
            conduit_core::CapabilityLifecycle::Revoked
        );
    }
}
