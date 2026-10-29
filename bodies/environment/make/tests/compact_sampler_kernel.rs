//! Actual checked Source/Plan and generic kernel with a finite pulse transcript.
//! This is CPU execution/representation evidence, not MCU memory closure.
use conduit_core as c;
use conduit_environment_make::NativeSoftwareSourceRecipe;
use conduit_environment_runtime::{
    appliance::{self, assigned::*, ApplianceBack},
    construction::compile,
};
use conduit_kernel::{self as k, scheduler::*, ValueStorage};
use conduit_plan_lowering::local_tables::DirectLocalKernelTables;
#[allow(
    dead_code,
    reason = "shared allocation probe supports other owning suites"
)]
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
mod plan {
    pub const PREFIX: &str = "fixture/compact-sampler";
}
#[path = "../../runtime/tests/appliance_support/mod.rs"]
mod appliance_support;
#[allow(
    dead_code,
    reason = "shared selected pulse binding also covers assigned projections"
)]
#[path = "../../runtime/tests/support/assignment.rs"]
mod assignment;
#[allow(
    dead_code,
    reason = "shared pulse provider supports additional admission projections"
)]
#[path = "../../runtime/tests/support/pulse.rs"]
mod pulse;
#[path = "../../runtime/tests/support/verified_metadata.rs"]
mod verified_metadata;
fn block_on<F: core::future::Future>(future: F) -> F::Output {
    use core::task::{Context, Poll, Waker};
    let mut future = std::pin::pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    match future.as_mut().poll(&mut context) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("finite transcript unexpectedly pending"),
    }
}
#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    channel: usize,
    sequence: u64,
    canonical: Vec<u8>,
}
fn run(scenario: usize, compact: bool) -> Vec<Snapshot> {
    let count = 3;
    let source = compile(count, 0).unwrap();
    let recipe = NativeSoftwareSourceRecipe::prepare(&source).unwrap();
    let texts: Vec<_> = recipe.resident_computations().collect();
    let offers = recipe.resident_offers();
    // SAFETY: independently checked immutable Make image from this exact recipe.
    let image = unsafe {
        conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(
            recipe.planning_image(),
        )
    }
    .unwrap();
    let values = if compact {
        appliance::prepare_compact_value_storage()
    } else {
        appliance::prepare_value_storage()
    }
    .unwrap();
    let storage = values.reserved_storage_bytes();
    assert_eq!(values.item_capacity(), 9);
    assert_eq!(values.byte_capacity(), 9329);
    if compact {
        let ordinary = appliance::value_storage_bytes().unwrap();
        assert_eq!(storage, appliance::compact_value_storage_bytes().unwrap());
        assert!(ordinary - storage >= 4096);
        eprintln!("sampler canonical_backing={ordinary} compact_backing={storage} saved={} physical_payload={} logical_quota=9329 slots=9",ordinary-storage,7*683+2*28);
    }
    let mut software = Some(Box::new(
        PreparedResidentSoftware::prepare_source(&recipe.spec(), &texts, values).unwrap(),
    ));
    let (ready, effects) = pulse::ready(scenario);
    let mut host = appliance_support::native_advertisement(&ready);
    for bytes in software
        .as_ref()
        .unwrap()
        .source_offers(&recipe.spec(), &offers)
        .unwrap()
        .iter()
    {
        host.capabilities
            .push(serde_json::from_slice(bytes).unwrap());
    }
    let plan = source
        .plan_local(&host, &["conduit.base/local@1".into()], &[pulse::grant()])
        .unwrap();
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&plan.fragments[0]).unwrap();
    let (mut owner, pulse_node) = pulse::bind_budget(ready, &plan, &lowered, count as u32).unwrap();
    let metadata = verified_metadata::from_plan(&plan);
    let tables = DirectLocalKernelTables::<15, 14, 1, 15, 14, 15>::prepare(
        &metadata,
        &metadata.preparation_metadata().fragments[0].fragment_id,
    )
    .unwrap();
    let prepared = PreparedResidentSoftware::bind_local_direct_retained(
        &mut software,
        tables,
        &image,
        &offers,
        (&host.host_id, &host.boot_id, host.offer_generation),
    )
    .unwrap();
    assert!(software.is_none());
    assert_eq!(prepared.compact_observation_edge.is_some(), compact);
    let edge = prepared.compact_observation_edge;
    let roles = prepared.roles;
    let mut selectors = prepared.selectors;
    let tables = prepared.tables.into_parts();
    let sign_bytes = tables.sign_bytes;
    let sign_items = tables.sign_items.max(
        u16::try_from((sign_bytes as usize).div_ceil(core::mem::size_of::<k::KernelEvent>()))
            .unwrap(),
    );
    let mut kernel = FixedScheduler::<
        ApplianceBack,
        k::HostedValueStore,
        k::HostedSignLog,
        15,
        14,
        1,
        14,
        15,
        14,
        15,
        15,
    >::new_boxed_with_active_counts_and_host_calls(
        15,
        14,
        tables.nodes,
        tables.cords,
        tables.routes,
        tables.calls,
        prepared.drivers,
        prepared.values,
        k::HostedSignLog::new(sign_items, sign_bytes).unwrap(),
    )
    .unwrap();
    if let Some(edge) = edge {
        // SAFETY: exact checked tables, actual initialized drivers and affine
        // values were moved together from this witness's returned preparation.
        let (_, binding) = allocation_probe::observe(|| unsafe { edge.bind(&mut kernel) }.unwrap());
        assert_eq!((binding.allocations, binding.reallocations), (0, 0));
    }
    let mut waits = 0;
    let (_, play) = allocation_probe::observe(|| {
        let mut drained = false;
        for _ in 0..count * 2000 {
            if let Some(request) = kernel.next_host_request() {
                let input = kernel.host_value(request.input.value).unwrap();
                let output = if request.node == pulse_node {
                    Some(
                        block_on(owner.invoke(request.node, request.call, request.request, input))
                            .unwrap(),
                    )
                } else if request.node == roles.tick {
                    waits += 1;
                    None
                } else {
                    selectors[usize::from(request.node.0)]
                        .as_mut()
                        .unwrap()
                        .select(input)
                        .unwrap()
                };
                let output = output.map(|bytes| {
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
        assert!(drained, "scenario={scenario} compact={compact}");
    });
    assert_eq!((play.allocations, play.reallocations), (0, 0));
    assert_eq!(waits, count);
    assert_eq!(effects.get(), count as usize * usize::from(scenario != 4));
    roles
        .publications
        .iter()
        .enumerate()
        .filter_map(|(channel, node)| {
            kernel.drivers()[usize::from(node.0)].with_publication_snapshot(|s| Snapshot {
                channel,
                sequence: s.sequence,
                canonical: s.canonical.to_vec(),
            })
        })
        .collect()
}
#[test]
fn checked_compact_edge_preserves_all_seven_sampling_trajectories_and_logical_grants() {
    for scenario in 0..=6 {
        assert_eq!(
            run(scenario, true),
            run(scenario, false),
            "scenario={scenario}"
        );
    }
}
