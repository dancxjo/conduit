//! Whole authenticated Source execution; CPU equipment and typed local Fore.
use crate::{
    neutral_factory::Kernel,
    neutral_workset::{self, layout},
};
use conduit_body as b;
use conduit_core as c;
use conduit_kernel::{scheduler::*, *};
use conduit_mcu_host_runtime::{
    execution::Policy,
    native_dispatch::{deadline, Dispatch},
    native_runner::{synchronous, CooperativeYield, NativeActivity, Observer, Runner},
};
use core::{cell::RefCell, future::Future, task::Poll};
struct View {
    pins: [c::AssignedForeRequirement; 12],
    validators: [c::BorrowedStructuredValueValidator<'static>; 12],
    staged: [bool; 3],
    counts: [usize; 9],
    closed: [bool; 9],
    steps: usize,
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
            counts: [0; 9],
            closed: [false; 9],
            steps: 0,
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
                self.counts[i] += 1;
                assert!(
                    matches!(i, 0 | 1 | 4 | 6),
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
    let actual_owned = parts.resources.host().clone();
    let actual = &actual_owned;
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
    let context = b::NativePreparationContext {
        bootstrap: parts.bootstrap,
        member: parts.member,
        actual,
        image_id: parts.configuration.image_id,
        configuration_id: parts.configuration.configuration_id,
        now_millis: (runtime.now)(),
    };
    let policy = Policy {
        preparation: parts.configuration.preparation,
        execution: parts.configuration.execution,
        active_worksets: 0,
        maximum_retained_bytes: 1024 * 1024,
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
    let prepared = crate::neutral_factory::prepare(
        parts.workspace,
        context,
        policy,
        residents,
        &mut software,
        &mut equipment.bus,
        &mut equipment.clock,
        &mut issuers.bus,
        &mut issuers.clock,
        operations,
    )
    .unwrap();
    let mut committed = prepared
        .admit::<&'static str>(parts.resources, &observations, observation_cost.peak_bytes)
        .unwrap();
    let mut started = committed
        .start::<{ crate::config::MAX_ASSIGNMENT_BYTES }, &'static str>(
            parts.workspace,
            context,
            policy,
            parts.resources,
        )
        .unwrap();
    assert!(
        committed
            .start::<{ crate::config::MAX_ASSIGNMENT_BYTES }, &'static str>(
                parts.workspace,
                context,
                policy,
                parts.resources
            )
            .is_err(),
        "Start cannot bind providers twice"
    );
    let (kernel, calls) = started.kernels_mut();
    let mut output = vec![0; conduit_i2c_transaction::contract::I2C_RESULT_VALUE_BYTES];
    let mut bus = synchronous::Service::new(&mut calls.buses[0], &mut output).unwrap();
    let mut clock = deadline::Service::new(&mut calls.clocks[0]);
    let mut view = View::new();
    let activity = NativeActivity::new();
    let pause = Yield;
    let cancelled = || false;
    let runner = Runner::new(&mut **kernel, &mut view, &pause, &cancelled).with_activity(&activity);
    let (result, cost) =
        crate::probe::observe(|| finish(runner.run_with_i2c_clock(&mut bus, &mut clock)));
    assert!(result.is_ok(), "whole Source runner refused: {result:?}");
    assert_eq!(
        cost.allocation_attempts, 0,
        "execution attempted allocation"
    );
    view.service(kernel).unwrap();
    assert_eq!(view.staged, [true; 3]);
    assert_eq!(view.counts, [1, 1, 0, 0, 1, 0, 1, 0, 0]);
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
    println!("whole_source76 authenticated_Start=true typed_inputs=3 typed_outputs=9 observed=4 all_outputs_closed=true native_dispatch=true execution_allocations=0 finite_component_only");
}
