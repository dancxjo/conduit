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
#[path = "appliance_support/software.rs"]
mod software_tests;
use conduit_core as c;
use conduit_environment_runtime::appliance::ApplianceBack as Driver;
use conduit_environment_runtime::{
    construction::compile, publication::PublicationChannel as Channel, CaptureBack,
    PreparedCaptureSelector,
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
    let mut software = software(&source);
    let mut host = appliance_support::advertisement(&source, &mut software, &ready);
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
        run(scenario, false);
    }
}
#[test]
fn full_authored_overnight_workload_drains_with_a_small_value_store() {
    // Deterministic CPU clock completions; no physical sixteen-hour claim.
    for scenario in 0..=6 {
        run(scenario, true);
    }
}
fn run(scenario: usize, overnight: bool) {
    let source = if overnight {
        conduit_environment_runtime::construction::compile_default().unwrap()
    } else {
        compile(1, 0).unwrap()
    };
    let count = source.count;
    let period_ms = source.period_ms;
    let (ready, effects) = pulse::ready(scenario);
    let (mut software, software_memory) = allocation_probe::observe(|| software(&source));
    if scenario == 0 {
        eprintln!(
            "environment_software_ready_peak={}",
            software_memory.peak_bytes
        );
    }
    let host = appliance_support::advertisement(&source, &mut software, &ready);
    let (plan, planning_memory) = allocation_probe::observe(|| {
        source
            .plan_local(&host, &["conduit.base/local@1".into()], &[pulse::grant()])
            .unwrap()
    });
    assert!(c::verify_plan(&plan));
    let (lowered, lowering_memory) = allocation_probe::observe(|| {
        conduit_plan_lowering::lowering::lower_plan_fragment(&plan.fragments[0]).unwrap()
    });
    if scenario == 0 {
        // Isolate copies from Source checking's large transient catalogue. These
        // are host heap observations, not an MCU memory-admission claim.
        let (graph_copy, graph_memory) = allocation_probe::observe(|| source.expanded.clone());
        let (host_copy, host_memory) = allocation_probe::observe(|| host.clone());
        let (plan_copy, plan_memory) = allocation_probe::observe(|| plan.clone());
        #[cfg(feature = "body-image")]
        {
            let body = conduit_body::Body::born(
                plan.source_document_id.clone(),
                plan.checked_plot_id.clone(),
                1,
                "sign/memory/born".into(),
            )
            .unwrap();
            let wake = body.wake(2, "sign/memory/wake".into()).unwrap().1;
            let body_plan = conduit_body::BodyPlan::seal(
                &wake,
                vec![conduit_body::BodyPlotPlan {
                    plot: wake.workset.plots()[0].clone(),
                    plan: plan.clone(),
                }],
            )
            .unwrap();
            let (result, validation_memory) =
                allocation_probe::observe(|| body_plan.validate_for(&wake));
            result.unwrap();
            // Revalidation may hash semantic chunks and sort bounded references;
            // it must not duplicate the appliance's rich nested Plan.
            assert!(
                validation_memory.peak_bytes < plan_memory.peak_bytes / 2,
                "Body validation copied rich Plan backing: {validation_memory:?}"
            );
            eprintln!(
                "environment_body_plan_validation_peak={} plan_copy_peak={}",
                validation_memory.peak_bytes, plan_memory.peak_bytes
            );
        }
        drop((graph_copy, host_copy, plan_copy));
        eprintln!(
            "environment_appliance_host_heap graph_copy_peak={} host_copy_peak={} plan_copy_peak={} ordinary_planning_peak={} lowering_peak={}",
            graph_memory.peak_bytes, host_memory.peak_bytes, plan_memory.peak_bytes,
            planning_memory.peak_bytes, lowering_memory.peak_bytes,
        );
    }
    assert_eq!((lowered.nodes.len(), lowered.cords.len()), (N, C));
    let (mut owner, pulse_node) =
        pulse::bind_budget(ready, &plan, &lowered, count.try_into().unwrap()).unwrap();
    let nodes = core::array::from_fn(|i| {
        let rich = lowered.node_specs[i];
        assert!(rich.input_cords[P..].iter().all(Option::is_none));
        conduit_kernel::scheduler::NodeSpec {
            input_cords: core::array::from_fn(|port| rich.input_cords[port]),
            maximum_step_fuel: rich.maximum_step_fuel,
        }
    });
    let cords = core::array::from_fn(|i| lowered.cords[i].spec);
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
    let mut values = conduit_environment_runtime::appliance::prepare_value_storage().unwrap();
    assert_eq!(
        values.reserved_storage_bytes(),
        conduit_environment_runtime::appliance::value_storage_bytes().unwrap()
    );
    let sign_bytes = lowered.sign_bytes;
    let sign_slots = lowered.sign_items.max(
        u16::try_from(
            (lowered.sign_bytes as usize).div_ceil(core::mem::size_of::<k::KernelEvent>()),
        )
        .unwrap(),
    );
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
    let (drivers, mut selectors, values) = {
        let wait = values
            .store(&c::encode_monotonic_duration(period_ms))
            .unwrap();
        let mut selectors: [Option<PreparedCaptureSelector>; N] = core::array::from_fn(|_| None);
        let mut drivers: [Option<Driver>; N] = core::array::from_fn(|_| None);
        for placement in &plan.fragments[0].startup_order {
            let i = usize::from(lowered.identity.node_for_placement(placement).unwrap().0);
            let gear = plan.fragments[0]
                .placements
                .iter()
                .find(|g| &g.placement_id == placement)
                .unwrap();
            drivers[i] = Some(if gear.kind_id.as_str() == conduit_time::TICK_KIND {
                Driver::Tick(conduit_time::TickBack::counted(wait, count).unwrap())
            } else if gear.kind_id.as_str() == conduit_pulse_capture::PULSE_READ_KIND {
                Driver::Native(CaptureBack::pulse(1024))
            } else {
                let prepared = software
                    .take_selected(&plan.fragments[0], placement)
                    .unwrap();
                selectors[i] = prepared.selector;
                prepared.driver
            });
        }
        (drivers.map(Option::unwrap), selectors, values)
    };
    let mut kernel = FixedScheduler::<_, _, _, N, C, P, C, R, C, N, N>::new_boxed_with_active_counts_and_host_calls(
        N, C,
        nodes,
        cords,
        routes,
        calls,
        drivers,
        values,
        k::HostedSignLog::new(sign_slots, sign_bytes).unwrap(),
    )
    .unwrap();
    let mut drained = false;
    let mut waits = 0_u64;
    let mut peak_items = 0;
    let mut peak_bytes = 0;
    let ((), allocation) = allocation_probe::observe(|| {
        for _ in 0..count * 2000 {
            if let Some(request) = kernel.next_host_request() {
                let input = kernel.host_value(request.input.value).unwrap();
                let bytes = if request.node == pulse_node {
                    Some(
                        block_on(owner.invoke(request.node, request.call, request.request, input))
                            .unwrap(),
                    )
                } else if request.node == tick_node {
                    assert_eq!(input, c::encode_monotonic_duration(period_ms));
                    waits += 1;
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
            let status = kernel.step().unwrap();
            peak_items = peak_items.max(kernel.values().used_items());
            peak_bytes = peak_bytes.max(kernel.values().used_bytes());
            if status == SchedulerStatus::Drained {
                drained = true;
                break;
            }
        }
    });
    assert!(drained);
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
    assert_eq!(effects.get(), count as usize * usize::from(scenario != 4));
    assert_eq!(waits, count);
    if overnight {
        eprintln!("overnight_post_step_peak_live_items={peak_items} overnight_post_step_peak_live_bytes={peak_bytes}");
        eprintln!("overnight_samples={count} period_ms={period_ms} logical_elapsed_ms={} value_slots=9 value_bytes=11716",waits*period_ms);
    }
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
                assert_eq!(snapshot.sequence, count);
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

fn software(
    source: &conduit_environment_runtime::construction::ApplianceSource,
) -> conduit_environment_runtime::appliance::software::ReadySoftwareBacks {
    conduit_environment_runtime::appliance::software::ReadySoftwareBacks::prepare(
        source,
        pulse::identity().host_id,
        pulse::identity().boot_id,
    )
    .unwrap()
}
