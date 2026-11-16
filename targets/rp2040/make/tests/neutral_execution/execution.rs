//! Whole authenticated Source execution; CPU equipment and typed local Fore.
use crate::{
    neutral_factory::Kernel,
    neutral_workset::{self, layout},
};
use conduit_core as c;
use conduit_kernel::{scheduler::*, *};
use conduit_mcu_host_runtime::{
    native_dispatch::{deadline, Dispatch},
    native_runner::{synchronous, CooperativeYield, NativeActivity, Observer, Runner},
};
use core::{cell::RefCell, future::Future, task::Poll};
struct View {
    pins: [c::AssignedForeRequirement; 12],
    validators: [c::BorrowedStructuredValueValidator<'static>; 12],
    staged: [bool; 3],
    frame_closed_without_value: bool,
    counts: [usize; 9],
    closed: [bool; 9],
    steps: usize,
    backing_known: bool,
}
impl View {
    fn new() -> Self {
        let all = layout::fore();
        let pins = core::array::from_fn(|i| {
            *all.iter()
                .find(|pin| {
                    pin.payload()[5..21]
                        == c::AssignedIdentity::from_text(crate::typed::PORTS[i].0).0
                })
                .unwrap()
        });
        assert_eq!(all.len(), 12);
        for (i, pin) in pins.iter().enumerate() {
            assert_eq!(
                pin.direction(),
                if i < 3 {
                    c::AssignedForeDirection::Input
                } else {
                    c::AssignedForeDirection::Output
                }
            );
        }
        Self {
            pins,
            validators: core::array::from_fn(|i| {
                c::BorrowedStructuredValueValidator::new(crate::typed::PORTS[i].1, 4096).unwrap()
            }),
            staged: [false; 3],
            frame_closed_without_value: false,
            counts: [0; 9],
            closed: [false; 9],
            steps: 0,
            backing_known: true,
        }
    }
    fn service(&mut self, kernel: &mut Kernel<'_, '_>) -> Result<(), SchedulerError> {
        self.steps += 1;
        assert!(
            self.steps < 100000,
            "finite protocol did not quiesce: counts={:?} closed={:?}",
            self.counts,
            self.closed
        );
        if !self.staged[2] && !self.frame_closed_without_value && self.counts[5] > 0 {
            let pin = self.pins[2];
            kernel.close_remote_input(RemoteEndpointId(pin.endpoint()), CordId(pin.cord()))?;
            self.frame_closed_without_value = true;
        }
        for i in 0..3 {
            if !self.staged[i] && (i != 2 || self.counts[4] > 0) {
                let pin = self.pins[i];
                let bytes = crate::typed::INPUTS[i];
                self.validators[i].validate(bytes).unwrap();
                assert!(matches!(
                    kernel.admit_remote_input(
                        RemoteEndpointId(pin.endpoint()),
                        CordId(pin.cord()),
                        0,
                        bytes
                    )?,
                    RemoteIngressOutcome::Accepted { sequence: 0 }
                ));
                kernel.close_remote_input(RemoteEndpointId(pin.endpoint()), CordId(pin.cord()))?;
                self.staged[i] = true;
            }
        }
        for i in 0..9 {
            let pin = self.pins[i + 3];
            let endpoint = RemoteEndpointId(pin.endpoint());
            let cord = CordId(pin.cord());
            if let Some(offer) = kernel.remote_egress_offer(endpoint, cord)? {
                let bytes = kernel.host_value(offer.value)?;
                let value = self.validators[i + 3].validate(bytes).unwrap();
                if i == 1 {
                    let observation = value
                        .variant_payload("observation")
                        .unwrap()
                        .expect("Source emitted a semantic observation");
                    for (name, expected) in [
                        ("temperature_centidegrees", 2508_i128),
                        ("pressure_q24_8_pa", 25767233),
                        ("humidity_q22_10_percent", 55588),
                    ] {
                        let field = observation.record_field(name).unwrap().unwrap();
                        let actual = i128::from_le_bytes(
                            field
                                .primitive_bytes("value/i128")
                                .unwrap()
                                .try_into()
                                .unwrap(),
                        );
                        assert_eq!(actual, expected, "Source compensation {name}");
                    }
                }
                if i == 0 {
                    assert_eq!(
                        value
                            .record_field("phase")
                            .unwrap()
                            .unwrap()
                            .primitive_bytes("value/u8")
                            .unwrap(),
                        &[14]
                    );
                }
                if i == 5 {
                    assert!(crate::FAIL_DISPLAY);
                    let bus_failure = value
                        .variant_payload("bus")
                        .unwrap()
                        .expect("typed initialization bus failure");
                    assert_eq!(bus_failure.variant_tag().unwrap(), "not-acknowledged");
                }
                self.counts[i] += 1;
                assert!(
                    matches!(i, 0 | 1)
                        || if crate::FAIL_DISPLAY {
                            i == 5
                        } else {
                            matches!(i, 4 | 6)
                        },
                    "unexpected refused/unrouted output {}",
                    crate::typed::PORTS[i + 3].0
                );
                kernel.remote_egress_accept(endpoint, cord, offer.sequence)?;
                kernel.remote_egress_delivered(endpoint, cord, offer.sequence)?;
            }
            self.closed[i] = kernel.remote_egress_terminal(endpoint, cord)?;
        }
        Ok(())
    }
}
// SAFETY: this installed fixture owns only its complete fixed Fore state and
// borrowed static validators. Binding moves it unchanged without effects or
// allocation; all input/output service happens after the enclosing Start.
unsafe impl conduit_mcu_host_runtime::execution_workset::PreparedLocalKernels for View {
    type Bound = Self;
    fn retained_backing_bytes(&self) -> Option<usize> {
        self.backing_known.then_some(core::mem::size_of::<Self>())
    }
    unsafe fn bind(self) -> Self {
        self
    }
}
impl Observer<Kernel<'_, '_>> for View {
    type Error = SchedulerError;
    fn before_step(&mut self, kernel: &mut Kernel<'_, '_>) -> Result<(), Self::Error> {
        self.service(kernel)
    }
    fn completed(&mut self, _: &mut Kernel<'_, '_>, _: Dispatch) -> Result<(), Self::Error> {
        Ok(())
    }
}
struct Yield;
impl CooperativeYield for Yield {
    async fn pause(&self) {
        let mut first = true;
        core::future::poll_fn(|cx| {
            if first {
                first = false;
                cx.waker().wake_by_ref();
                Poll::Pending
            } else {
                Poll::Ready(())
            }
        })
        .await
    }
}
fn finish<F: Future>(future: F) -> F::Output {
    let mut future = core::pin::pin!(future);
    let mut context = core::task::Context::from_waker(core::task::Waker::noop());
    for _ in 0..300000 {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
    }
    panic!("bounded CPU runner exceeded poll ceiling")
}
#[allow(clippy::too_many_arguments)]
pub fn run<
    I: conduit_i2c_transaction::I2cProvider,
    D: conduit_monotonic_wait::clock_at::owner::MonotonicDeadlineProvider,
>(
    root: &mut conduit_mcu_host_runtime::root::Root<'_, { crate::config::MAX_ASSIGNMENT_BYTES }>,
    equipment: &mut crate::neutral_equipment::Equipment<I, D>,
    residents: &mut crate::resident::generated::Initialized,
    issuers: &mut crate::neutral_authority::Issuers,
    operations: &crate::neutral_authority::Operations,
    runtime: &crate::neutral_preflight::Runtime,
) {
    let parts = root.execution_parts().unwrap();
    let actual = parts.resources.host();
    let (observations, observation_cost) = crate::probe::observe(|| {
        actual
            .resources
            .iter()
            .map(|pool| c::ResourceObservation {
                host_id: actual.host_id.clone(),
                boot_id: actual.boot_id.clone(),
                offer_generation: actual.offer_generation,
                pool_id: pool.pool_id.clone(),
                class_id: pool.class_id.clone(),
                health: c::ResourceHealth::Ready,
                unreserved_units: pool.capacity_units,
                utilized_units: 0,
                sign_id: format!("cpu/observed/{}", pool.pool_id.as_str()).into(),
            })
            .collect::<Vec<_>>()
    });
    let scope = crate::neutral_execution::RootScope {
        workspace: parts.workspace,
        bootstrap: parts.bootstrap,
        member: parts.member,
        configuration: parts.configuration,
        now_millis: runtime.now,
    };
    let publisher = RefCell::new(None);
    let mut software = conduit_mcu_host_runtime::execution_factory::SoftwareInventory::<
        { neutral_workset::N },
    >::empty();
    // SAFETY: original installed definitions/identities are emitted together;
    // this CPU fixture exclusively owns their preparation allocator interval.
    unsafe {
        software
            .initialize_by(
                |n| neutral_workset::source::definition_at(n, &[]),
                |n| neutral_workset::source::identity_at(n, &[]),
                &publisher,
                conduit_environment_runtime::publication::PublicationSequence::prepare(0),
                runtime.now,
                runtime.allocated_bytes,
                neutral_workset::source::CONFIGURATION,
                None,
            )
            .unwrap();
    }
    let mut output = vec![0; conduit_i2c_transaction::contract::I2C_RESULT_VALUE_BYTES];
    let pins = layout::fore();
    check_cold_refusals(&pins, equipment, parts.resources);
    let fore =
        crate::neutral_execution::PreparedFore::new(View::new(), &pins, &mut output).unwrap();
    let mut committed = crate::neutral_execution::admit(
        scope,
        parts.resources,
        equipment,
        residents,
        &mut software,
        issuers,
        operations,
        fore,
        &observations,
        observation_cost.peak_bytes,
        0,
        1024 * 1024,
    )
    .unwrap();
    let mut started =
        crate::neutral_execution::start(&mut committed, scope, parts.resources, 0, 1024 * 1024)
            .unwrap();
    assert!(
        crate::neutral_execution::start(&mut committed, scope, parts.resources, 0, 1024 * 1024)
            .is_err(),
        "Start cannot bind providers twice"
    );
    let ((kernel, calls), services) = started.kernels_mut();
    let (view, output) = services.parts_mut();
    let mut bus = synchronous::Service::new(&mut calls.buses[0], output).unwrap();
    let mut clock = deadline::Service::new(&mut calls.clocks[0]);
    let activity = NativeActivity::new();
    let pause = Yield;
    let cancelled = || false;
    let runner = Runner::new(&mut **kernel, view, &pause, &cancelled).with_activity(&activity);
    let (result, cost) =
        crate::probe::observe(|| finish(runner.run_with_i2c_clock(&mut bus, &mut clock)));
    assert!(result.is_ok(), "whole Source runner refused: {result:?}");
    assert_eq!(
        cost.allocation_attempts, 0,
        "execution attempted allocation"
    );
    view.service(kernel).unwrap();
    assert_eq!(view.staged, [true, true, !crate::FAIL_DISPLAY]);
    assert_eq!(view.frame_closed_without_value, crate::FAIL_DISPLAY);
    assert_eq!(
        view.counts,
        if crate::FAIL_DISPLAY {
            [1, 1, 0, 0, 0, 1, 0, 0, 0]
        } else {
            [1, 1, 0, 0, 1, 0, 1, 0, 0]
        }
    );
    assert_eq!(view.closed, [true; 9]);
    assert_eq!(bus.bound_owner().inspection().in_flight_operations, 0);
    assert_eq!(clock.bound_owner().inspection().in_flight_operations, 0);
    assert_eq!(
        bus.bound_owner().inspection().lifecycle,
        c::CapabilityLifecycle::Revoked
    );
    assert_eq!(
        clock.bound_owner().inspection().lifecycle,
        c::CapabilityLifecycle::Revoked
    );
    println!("whole_source76 authenticated_Start=true typed_inputs=3 typed_outputs=9 display_nack={} frame_sent={} outputs={:?} all_outputs_closed=true native_dispatch=true execution_allocations=0 finite_component_only",crate::FAIL_DISPLAY,view.staged[2],view.counts);
}

fn check_cold_refusals<I, D>(
    pins: &[c::AssignedForeRequirement],
    equipment: &crate::neutral_equipment::Equipment<I, D>,
    resources: &c::ResourceAdmissionOwner,
) {
    let before = resources.clone();
    for mode in 0..5 {
        let mut changed = pins.to_vec();
        let mut output = vec![0; conduit_i2c_transaction::contract::I2C_RESULT_VALUE_BYTES];
        match mode {
            0 => {
                changed.pop();
            }
            1 => {
                changed[1] = changed[0];
            }
            2 => {
                let pin = changed[0];
                let mut abi: [u8; 32] = pin.payload()[21..].try_into().unwrap();
                abi[0] ^= 1;
                changed[0] = c::AssignedForeRequirement::new(
                    pin.endpoint(),
                    pin.cord(),
                    pin.direction(),
                    c::AssignedIdentity(pin.payload()[5..21].try_into().unwrap()),
                    abi,
                );
            }
            3 => {
                output.pop();
            }
            _ => {}
        }
        let mut observer = View::new();
        observer.backing_known = mode != 4;
        let result = crate::neutral_execution::PreparedFore::new(observer, &changed, &mut output);
        assert!(result.is_err(), "invalid Fore/buffer accepted mode={mode}");
        assert_eq!(resources, &before, "cold refusal changed admission");
        assert!(
            equipment.bus.is_some() && equipment.clock.is_some(),
            "cold refusal bound provider"
        );
    }
}
