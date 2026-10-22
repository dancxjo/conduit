//! A transcript is the only simulated device. Source, publication, binding and
//! execution are the ordinary implementations; no fixture observer actors.
#![cfg(feature = "construction")]
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[allow(
    dead_code,
    reason = "the shared pulse transcript also supports assigned projections"
)]
#[path = "support/assignment.rs"]
mod assignment;
mod plan {
    pub const PREFIX: &str = "fixture/environment";
}
mod appliance_support;
#[allow(
    dead_code,
    reason = "the shared provider also exercises assignment admission"
)]
#[path = "support/pulse.rs"]
mod pulse;
use conduit_core as c;
use conduit_environment_runtime::appliance::ApplianceBack as Driver;
use conduit_environment_runtime::{
    construction::compile,
    observation::ObservationContract,
    publication::{PublicationChannel as Channel, PublicationContract},
    CaptureBack, PreparedCaptureSelector,
};
use conduit_kernel::{
    self as k,
    scheduler::{FixedScheduler, SchedulerStatus},
    ValueStorage,
};
const N: usize = 15;
const C: usize = 14;
const P: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const R: usize = N * P;
#[test]
fn a_ready_capability_does_not_replace_authority_or_missing_implementations() {
    let source = compile(1, 0).unwrap();
    let (ready, effects) = pulse::ready(0);
    let mut host = appliance_support::advertisement(&source, &ready);
    let bases = ["conduit.base/local@1".into()];
    assert!(source.plan_local(&host, &bases, &[]).is_err());
    let mut wrong = pulse::grant();
    wrong.boot_id = "wrong/boot".into();
    assert!(source.plan_local(&host, &bases, &[wrong]).is_err());
    source.plan_local(&host, &bases, &[pulse::grant()]).unwrap();
    host.capabilities
        .retain(|offer| offer.kind_id.as_str() != Channel::Reading.kind_id());
    assert!(source.plan_local(&host, &bases, &[pulse::grant()]).is_err());
    assert_eq!(effects.get(), 0);
}
#[test]
fn authored_appliance_commits_all_sensor_outcomes_through_the_production_kernel() {
    for scenario in 0..=6 {
        run(scenario);
    }
}
fn run(scenario: usize) {
    let source = compile(1, 0).unwrap();
    let (ready, effects) = pulse::ready(scenario);
    let host = appliance_support::advertisement(&source, &ready);
    let plan = source
        .plan_local(&host, &["conduit.base/local@1".into()], &[pulse::grant()])
        .unwrap();
    assert!(c::verify_plan(&plan));
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&plan.fragments[0]).unwrap();
    assert_eq!((lowered.nodes.len(), lowered.cords.len()), (N, C));
    let (mut owner, pulse_node) = pulse::bind(ready, &plan, &lowered).unwrap();
    let mut capacities = [1024; 32];
    capacities[..2].fill(4096);
    let mut values = k::HostedValueStore::new_with_slot_capacities(&capacities, 38912).unwrap();
    let wait = values.store(&0_u64.to_le_bytes()).unwrap();
    let mut selectors: [Option<PreparedCaptureSelector>; N] = core::array::from_fn(|_| None);
    let mut drivers: [Option<Driver>; N] = core::array::from_fn(|_| None);
    for placement in &plan.fragments[0].startup_order {
        let i = usize::from(lowered.identity.node_for_placement(placement).unwrap().0);
        let gear = plan.fragments[0]
            .placements
            .iter()
            .find(|g| &g.placement_id == placement)
            .unwrap();
        drivers[i] = Some(
            if let Some(channel) = Channel::ALL
                .into_iter()
                .find(|c| c.kind_id() == gear.kind_id.as_str())
            {
                Driver::publication(&PublicationContract::prepare(channel).unwrap(), 1).unwrap()
            } else if gear.kind_id.as_str() == conduit_time::TICK_KIND {
                Driver::Tick(conduit_time::TickBack::counted(wait, 1).unwrap())
            } else if gear.kind_id.as_str() == conduit_time::TICK_TRIGGER_KIND {
                Driver::Trigger(conduit_time::TickTriggerBack::default())
            } else if gear.kind_id.as_str() == conduit_pulse_capture::PULSE_READ_KIND {
                Driver::Native(CaptureBack::pulse(1024))
            } else if gear.kind_id.as_str() == conduit_environment_runtime::observation::KIND {
                Driver::Native(
                    CaptureBack::observation(&ObservationContract::prepare().unwrap()).unwrap(),
                )
            } else if let Some(c::ConfigurationValue::Text(program)) = gear
                .configuration
                .iter()
                .find(|c| c.key == "program")
                .map(|c| &c.value)
            {
                Driver::Native(
                    CaptureBack::expression(
                        &conduit_plot::PortableExpressionProgram::from_canonical_hex(program)
                            .unwrap(),
                        c::PortTemporal::Flow { closes: true },
                    )
                    .unwrap(),
                )
            } else {
                let Some(c::ConfigurationValue::Text(selector)) = gear
                    .configuration
                    .iter()
                    .find(|c| c.key == "selector")
                    .map(|c| &c.value)
                else {
                    panic!("uninstalled Kind")
                };
                selectors[i] = Some(
                    PreparedCaptureSelector::prepare(
                        c::StructuredSelector::from_canonical_hex(selector).unwrap(),
                        1024,
                        1024,
                    )
                    .unwrap(),
                );
                Driver::Native(CaptureBack::selector(1024))
            },
        );
    }
    let mut routes = k::FixedRoutes::<R, C>::new(P as u16);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .unwrap();
    }
    routes.seal().unwrap();
    let mut calls = k::FixedHostCallBindings::<N>::new(1);
    for call in &lowered.host_calls {
        calls.install(call.node, call.binding).unwrap();
    }
    calls.seal().unwrap();
    let sign_slots = lowered.sign_items.max(
        u16::try_from(
            (lowered.sign_bytes as usize).div_ceil(core::mem::size_of::<k::KernelEvent>()),
        )
        .unwrap(),
    );
    let mut kernel = FixedScheduler::<_, _, _, N, C, P, C, R, C, N, N>::new_with_host_calls(
        core::array::from_fn(|i| lowered.node_specs[i]),
        core::array::from_fn(|i| lowered.cords[i].spec),
        routes,
        calls,
        drivers.map(Option::unwrap),
        values,
        k::HostedSignLog::new(sign_slots, lowered.sign_bytes).unwrap(),
    )
    .unwrap();
    let tick_node = lowered
        .nodes
        .iter()
        .find(|node| {
            plan.fragments[0].placements.iter().any(|gear| {
                gear.placement_id == node.placement_id
                    && gear.kind_id.as_str() == conduit_time::TICK_KIND
            })
        })
        .unwrap()
        .node;
    let mut drained = false;
    let ((), allocation) = allocation_probe::observe(|| {
        for _ in 0..2000 {
            if let Some(request) = kernel.next_host_request() {
                let input = kernel.host_value(request.input.value).unwrap();
                let bytes = if request.node == pulse_node {
                    Some(
                        block_on(owner.invoke(request.node, request.call, request.request, input))
                            .unwrap(),
                    )
                } else if request.node == tick_node {
                    None
                } else {
                    selectors[usize::from(request.node.0)]
                        .as_mut()
                        .unwrap()
                        .select(input)
                        .unwrap()
                };
                let output = bytes.map(|bytes| {
                    k::BoundedValueRef::new(kernel.store_host_value(bytes).unwrap(), 1024).unwrap()
                });
                kernel
                    .complete_host_call(
                        request.node,
                        request.request,
                        k::HostCallOutcome {
                            disposition: k::HostCallDisposition::Completed,
                            output,
                            failure: None,
                        },
                    )
                    .unwrap();
            }
            if kernel.step().unwrap() == SchedulerStatus::Drained {
                drained = true;
                break;
            }
        }
    });
    assert!(drained);
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
    assert_eq!(effects.get(), usize::from(scenario != 4));
    let expected = match scenario {
        0 | 1 | 5 | 6 => Channel::Reading,
        2 => Channel::CaptureTimeout,
        3 => Channel::ProviderFailure,
        _ => Channel::GeometryRefusal,
    };
    for driver in kernel.drivers() {
        if let Driver::Publication(back) = driver {
            if let Some(snapshot) = back.snapshot() {
                assert_eq!(snapshot.channel, expected);
                assert_eq!(snapshot.sequence, 1);
                c::validate_canonical_structured_value(snapshot.canonical).unwrap();
            }
        }
    }
    assert_eq!(
        kernel
            .drivers()
            .iter()
            .filter(
                |driver| matches!(driver,Driver::Publication(back) if back.snapshot().is_some())
            )
            .count(),
        1
    );
}
fn block_on<F: core::future::Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    match future.as_mut().poll(&mut context) {
        std::task::Poll::Ready(value) => value,
        std::task::Poll::Pending => panic!("bounded transcript must finish immediately"),
    }
}
