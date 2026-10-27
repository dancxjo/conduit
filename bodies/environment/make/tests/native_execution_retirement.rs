//! CPU execution of the exact native worker/storage/service under initialized
//! transcript providers. No physical sensor, Body start or target RAM claim.
#![allow(unexpected_cfgs)]
extern crate alloc;
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core as c;
use conduit_environment_runtime::{
    appliance::{assigned::*, software::ReadySoftwareBacks, ApplianceBack},
    construction::compile,
    CaptureBack,
};
use conduit_kernel::{self as k, ValueStorage};
use conduit_monotonic_wait::*;
use conduit_pulse_capture::PulseReadContract;
use std::{cell::Cell, collections::BTreeMap, rc::Rc};
#[allow(
    dead_code,
    unused_imports,
    reason = "same native policy also has network/display entrances unused by this sampler proof"
)]
#[path = "../../../../targets/esp32/proof/tm1637/src/native_authority.rs"]
mod native_authority;
#[allow(
    dead_code,
    reason = "same storage owner has assigned preparation entrances"
)]
#[path = "../../../../targets/esp32/proof/tm1637/src/native_kernel_storage.rs"]
mod native_kernel_storage;
#[allow(dead_code, reason = "same native retirement also covers raw display")]
#[path = "../../../../targets/esp32/proof/tm1637/src/native_retirement.rs"]
mod native_retirement;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_service.rs"]
mod native_service;
#[allow(
    dead_code,
    reason = "same worker has additional observer/control entrances"
)]
#[path = "../../../../targets/esp32/proof/tm1637/src/native_worker.rs"]
mod native_worker;
// This proof never uses network policy. Reject every attempted access rather
// than supplying synthetic retirement evidence for that separate owner.
#[allow(dead_code)]
mod native_play {
    pub struct RetiredNetworkControl;
    impl RetiredNetworkControl {
        pub fn active(&self) -> &conduit_core::ActivePlayIdentity {
            panic!("no network retirement fixture")
        }
        pub fn evidence_is_committed(&self) -> bool {
            panic!("no network evidence fixture")
        }
        pub fn matches_active(&self, _: &conduit_core::ActivePlayIdentity) -> bool {
            panic!("no network scope fixture")
        }
    }
}
mod plan {
    pub const PREFIX: &str = "fixture/material-retirement";
}
#[allow(
    dead_code,
    reason = "shared provider also exercises assigned admission"
)]
#[path = "../../runtime/tests/support/pulse/provider.rs"]
mod pulse;
struct Wait(Rc<Cell<usize>>);
impl MonotonicWaitProvider for Wait {
    async fn wait(&mut self, _: u64) -> Result<(), WaitProviderFailure> {
        self.0.set(self.0.get() + 1);
        Ok(())
    }
}
type DeviceExecution = native_worker::Execution<pulse::Provider, Wait>;
struct Fixture {
    execution: Option<Box<DeviceExecution>>,
    active: c::ActivePlayIdentity,
    effects: Rc<Cell<usize>>,
    waits: Rc<Cell<usize>>,
}
fn fixture() -> Fixture {
    let source = compile(3, 250).unwrap();
    let (pulse, effects) = pulse::ready(0);
    let native = pulse::identity();
    let mut issuer =
        native_authority::RootIssuer::fixture(native.host_id.clone(), native.boot_id.clone());
    let calls = Rc::new(Cell::new(0));
    let wait_identity = WaitNativeIdentity {
        host_id: native.host_id.clone(),
        boot_id: native.boot_id.clone(),
        base_id: "fixture/wait-base".into(),
        provider_instance_id: "fixture/wait-provider".into(),
        provider_generation: 1,
        resource_pool_id: "fixture/wait-resource".into(),
        resource_generation_id: c::ResourceGenerationId("fixture/wait/1".into()),
        envelope_id: "fixture/wait-envelope".into(),
        artifact_id: "fixture/wait-artifact".into(),
        implementation_id: "fixture/wait@1".into(),
        base_implementation_id: "fixture/wait-base@1".into(),
    };
    // SAFETY: inert CPU timer retains only the counted state; it never touches hardware.
    let wait = unsafe {
        ReadyMonotonicWaitBase::new(
            wait_identity.clone(),
            WaitAttachment {
                generation: 1,
                resource_bytes: 64,
                maximum_duration_ms: 1000,
            },
            Wait(calls.clone()),
        )
    }
    .unwrap();
    let grants = [
        issuer.grant_pulse(pulse.native_identity()).unwrap(),
        issuer.grant_wait(wait.native_identity()).unwrap(),
    ];
    let mut host = c::HostAdvertisement {
        protocol_version: c::PROTOCOL_VERSION,
        host_id: native.host_id.clone(),
        boot_id: native.boot_id.clone(),
        offer_generation: c::OfferGeneration(1),
        profile: "fixture/root-policy".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let contract = PulseReadContract::prepare().unwrap();
    pulse.append_to_advertisement(&mut host, &contract).unwrap();
    wait.append_to_advertisement(&mut host).unwrap();
    let mut software =
        ReadySoftwareBacks::prepare(&source, native.host_id, native.boot_id).unwrap();
    software.append_to_advertisement(&mut host).unwrap();
    let recipe = conduit_environment_make::NativeSoftwareSourceRecipe::prepare(&source).unwrap();
    // SAFETY: these bytes were just checked and encoded by trusted Make.
    let image = unsafe {
        conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(
            recipe.planning_image(),
        )
    }
    .unwrap();
    let hosts = [host];
    let choices = conduit_planner::indexed_source::default_placements(&image, &hosts).unwrap();
    let empty = BTreeMap::new();
    let lines = BTreeMap::new();
    let queues = source
        .expanded
        .connections
        .iter()
        .map(|cord| {
            let reading = source.expanded.gears.iter().any(|g| {
                g.gear_id == cord.sink_gear_id
                    && g.kind_id.as_str()
                        == conduit_environment_runtime::publication::PublicationChannel::Reading
                            .kind_id()
            });
            let limit = [&cord.source_gear_id, &cord.sink_gear_id]
                .into_iter()
                .map(|gear| {
                    let choice = &choices.by_gear[gear];
                    hosts[0]
                        .capabilities
                        .iter()
                        .find(|offer| offer.capability_id == choice.capability_id)
                        .unwrap()
                        .limits
                        .max_queue_bytes
                })
                .min()
                .unwrap();
            (
                (
                    cord.source_gear_id.clone(),
                    cord.source_port_id.clone(),
                    cord.sink_gear_id.clone(),
                    cord.sink_port_id.clone(),
                ),
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: limit.min(if reading {
                        conduit_environment_runtime::publication::PublicationChannel::Reading
                            .maximum_bytes() as u32
                    } else {
                        1024
                    }),
                },
            )
        })
        .collect();
    let compact = conduit_planner::indexed_source::plan_compact_with_connection_limits(
        &image,
        &hosts,
        &choices,
        &["conduit.base/local@1".into()],
        conduit_planner::PlanningOptions {
            connection_bases: &empty,
            line_candidates: &lines,
            connection_item_capacity: 1,
            connection_byte_capacity: 1024,
            authority_grants: &grants,
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &queues,
    )
    .unwrap();
    let metadata = compact.into_verified_metadata().unwrap();
    let fragment = &metadata.preparation_metadata().fragments[0];
    let fragment_id = fragment.fragment_id.clone();
    let placement = |kind: &str| {
        fragment
            .placements
            .iter()
            .find(|g| g.kind_id.as_str() == kind)
            .unwrap()
            .placement_id
            .clone()
    };
    let pulse_placement = placement(conduit_pulse_capture::PULSE_READ_KIND);
    let wait_placement = placement(conduit_time::TICK_KIND);
    let verified = conduit_plan_lowering::local_tables::VerifiedLocalLowering::prepare(
        &metadata,
        &fragment_id,
    )
    .unwrap();
    let lowered = verified.lowered();
    let active = issuer
        .reserve_verified_play(&metadata, &fragment_id)
        .unwrap();
    let cadence = conduit_time::TickConfiguration {
        count: 3,
        period_ms: 250,
    };
    let mut pulse = Some(pulse);
    let mut wait = Some(wait);
    let mut native = Some(
        issuer
            .bind_verified_native(
                &mut pulse,
                &mut wait,
                native_authority::VerifiedNativeInput {
                    plan: &metadata,
                    fragment: &fragment_id,
                    lowered,
                    active: &active,
                    cadence: &cadence,
                    pulse: &pulse_placement,
                    wait: &wait_placement,
                    contract: &contract,
                },
            )
            .unwrap(),
    );
    let spec = recipe.spec();
    let texts = recipe.resident_computations().collect::<Vec<_>>();
    let offers = recipe.resident_offers();
    let software = PreparedResidentSoftware::prepare_source(
        &spec,
        &texts,
        conduit_environment_runtime::appliance::prepare_value_storage().unwrap(),
    )
    .unwrap();
    let mut software = Some(Box::new(software));
    let scope = (
        &hosts[0].host_id,
        &hosts[0].boot_id,
        hosts[0].offer_generation,
    );
    let prepared = PreparedResidentSoftware::bind_local_retained(
        &mut software,
        &verified,
        &image,
        &offers,
        scope,
    )
    .unwrap();
    // The native preparation's exact physical-slot calculation: retain the
    // complete semantic byte/item budgets on this host ABI too.
    let slots = usize::from(lowered.sign_items)
        .max((lowered.sign_bytes as usize).div_ceil(core::mem::size_of::<k::KernelEvent>()));
    let signs = k::HostedSignLog::new(slots.try_into().unwrap(), lowered.sign_bytes).unwrap();
    let backing = native_kernel_storage::preclaim_local_backing().unwrap();
    let backing_address = backing.as_ptr();
    let kernel =
        native_kernel_storage::Kernel::new_boxed_with_storage_and_active_counts_and_host_calls(
            Some(backing),
            NODES,
            CORDS,
            prepared.tables.nodes,
            prepared.tables.cords,
            prepared.tables.routes,
            prepared.tables.calls,
            prepared.drivers,
            prepared.values,
            signs,
        )
        .unwrap();
    assert_eq!(
        &*kernel as *const native_kernel_storage::Kernel,
        backing_address
    );
    let mut storage = Some(native_kernel_storage::PreparedKernel {
        kernel,
        selectors: Box::new(prepared.selectors),
        roles: prepared.roles,
    });
    let (execution, memory) = allocation_probe::observe(|| {
        Some(DeviceExecution::boxed_retained(&mut storage, &mut native).unwrap())
    });
    assert_eq!(
        memory.retained_bytes(),
        core::mem::size_of::<DeviceExecution>()
    );
    assert_eq!((memory.allocations, memory.reallocations), (1, 0));
    println!(
        "native_execution_layout pointer_bits={} inline={} construction_heap={} peak={}",
        usize::BITS,
        core::mem::size_of::<DeviceExecution>(),
        memory.retained_bytes(),
        memory.peak_bytes
    );
    Fixture {
        execution,
        active,
        effects,
        waits: calls,
    }
}
fn run(execution: &mut DeviceExecution) -> native_worker::Outcome {
    use core::{
        future::Future,
        pin::pin,
        task::{Context, Poll, Waker},
    };
    let mut future = pin!(execution.run(core::future::pending(), |_| {}));
    let mut context = Context::from_waker(Waker::noop());
    for _ in 0..10000 {
        if let Poll::Ready(outcome) = future.as_mut().poll(&mut context) {
            return outcome;
        }
    }
    panic!("finite transcript did not terminate")
}
fn expressions(kernel: &native_kernel_storage::Kernel) -> Vec<usize> {
    kernel
        .drivers()
        .iter()
        .filter_map(|driver| match driver {
            ApplianceBack::Native(CaptureBack::Expression(back)) => {
                Some(&**back as *const _ as usize)
            }
            _ => None,
        })
        .collect()
}
#[test]
fn normal_retirement_returns_original_kernel_software_backing_selectors_and_signs() {
    let mut f = fixture();
    let execution = f.execution.as_mut().unwrap();
    assert_eq!(run(execution).exit, native_worker::Exit::Drained);
    assert_eq!((f.effects.get(), f.waits.get()), (3, 3));
    assert!(execution.storage.retirement_ready());
    let kernel = &*execution.storage.kernel;
    let address = kernel as *const _ as usize;
    let software = expressions(kernel);
    let signs = kernel.signs().events().collect::<Vec<_>>();
    let values = kernel.values().used_bytes();
    let selectors = execution.storage.selectors.as_ptr();
    let roles = execution.storage.roles;
    let (retired, memory) = allocation_probe::observe(|| {
        DeviceExecution::retire_with_material_retained(&mut f.execution).unwrap()
    });
    assert_eq!(
        (memory.allocations, memory.reallocations),
        (0, 0),
        "retirement must transfer existing backing: {memory:?}"
    );
    assert!(f.execution.is_none());
    assert!(retired.providers.matches_active(&f.active));
    assert_eq!(retired.material.kernel() as *const _ as usize, address);
    assert_eq!(expressions(retired.material.kernel()), software);
    assert_eq!(retired.material.selectors().as_ptr(), selectors);
    assert_eq!(retired.material.roles(), roles);
    assert_eq!(retired.material.kernel().values().used_bytes(), values);
    assert_eq!(
        retired
            .material
            .kernel()
            .signs()
            .events()
            .collect::<Vec<_>>(),
        signs
    );
    let parts = retired.material.into_parts();
    assert!(!parts.execution.cancelled);
    assert_eq!(parts.execution.signs.events().collect::<Vec<_>>(), signs);
    assert_eq!(parts.execution.values.used_bytes(), values);
    assert_eq!(parts.selectors.as_ptr(), selectors);
    assert_eq!(parts.roles, roles);
    assert!(DeviceExecution::retire_with_material_retained(&mut f.execution).is_err());
}
#[test]
fn live_execution_refusal_preserves_every_address_then_original_can_run() {
    let mut f = fixture();
    assert!(!f.execution.as_ref().unwrap().storage.retirement_ready());
    let execution = &**f.execution.as_ref().unwrap() as *const _ as usize;
    let kernel = &*f.execution.as_ref().unwrap().storage.kernel as *const _ as usize;
    let selectors = f.execution.as_ref().unwrap().storage.selectors.as_ptr();
    assert_eq!(
        DeviceExecution::retire_with_material_retained(&mut f.execution).err(),
        Some("execution-cleanup-unconfirmed")
    );
    assert_eq!(
        &**f.execution.as_ref().unwrap() as *const _ as usize,
        execution
    );
    assert_eq!(
        &*f.execution.as_ref().unwrap().storage.kernel as *const _ as usize,
        kernel
    );
    assert_eq!(
        f.execution.as_ref().unwrap().storage.selectors.as_ptr(),
        selectors
    );
    assert_eq!((f.effects.get(), f.waits.get()), (0, 0));
    assert_eq!(
        run(f.execution.as_mut().unwrap()).exit,
        native_worker::Exit::Drained
    );
    DeviceExecution::retire_with_material_retained(&mut f.execution).unwrap();
}
#[test]
fn cancelled_retirement_preserves_cancellation_and_compatibility_returns_actual_providers() {
    let mut f = fixture();
    assert_eq!(
        f.execution.as_mut().unwrap().cancel_held().unwrap().exit,
        native_worker::Exit::Cancelled
    );
    let retired = DeviceExecution::retire_with_material_retained(&mut f.execution).unwrap();
    assert!(retired.providers.matches_active(&f.active));
    assert!(retired.material.into_parts().execution.cancelled);
    assert_eq!((f.effects.get(), f.waits.get()), (0, 0));
    let mut f = fixture();
    assert_eq!(
        run(f.execution.as_mut().unwrap()).exit,
        native_worker::Exit::Drained
    );
    let providers = DeviceExecution::retire_retained(&mut f.execution).unwrap();
    assert!(providers.matches_active(&f.active));
    assert!(f.execution.is_none());
    assert_eq!((f.effects.get(), f.waits.get()), (3, 3));
}

#[test]
fn abandoned_started_execution_must_quiesce_before_returning_material() {
    use core::{
        future::Future,
        pin::pin,
        task::{Context, Poll, Waker},
    };
    let mut f = fixture();
    let execution_address = &**f.execution.as_ref().unwrap() as *const _ as usize;
    let kernel_address = &*f.execution.as_ref().unwrap().storage.kernel as *const _ as usize;
    {
        let execution = f.execution.as_mut().unwrap();
        let mut future = pin!(execution.run(core::future::pending(), |_| {}));
        let mut context = Context::from_waker(Waker::noop());
        for _ in 0..1000 {
            assert!(matches!(future.as_mut().poll(&mut context), Poll::Pending));
            if f.effects.get() > 0 {
                break;
            }
        }
        assert!(f.effects.get() > 0);
    } // Real invocation futures are dropped; cleanup has not been accepted.
    let signs = f
        .execution
        .as_ref()
        .unwrap()
        .storage
        .kernel
        .signs()
        .events()
        .collect::<Vec<_>>();
    let pending = f
        .execution
        .as_ref()
        .unwrap()
        .storage
        .kernel
        .pending_host_call_count();
    let effects = (f.effects.get(), f.waits.get());
    assert_eq!(
        DeviceExecution::retire_with_material_retained(&mut f.execution).err(),
        Some("execution-cleanup-unconfirmed")
    );
    assert_eq!(
        &**f.execution.as_ref().unwrap() as *const _ as usize,
        execution_address
    );
    assert_eq!(
        &*f.execution.as_ref().unwrap().storage.kernel as *const _ as usize,
        kernel_address
    );
    assert_eq!(
        f.execution
            .as_ref()
            .unwrap()
            .storage
            .kernel
            .signs()
            .events()
            .collect::<Vec<_>>(),
        signs
    );
    assert_eq!(
        f.execution
            .as_ref()
            .unwrap()
            .storage
            .kernel
            .pending_host_call_count(),
        pending
    );
    let cleanup = run(f.execution.as_mut().unwrap());
    assert!(matches!(cleanup.exit, native_worker::Exit::Failed(_)));
    assert_eq!(cleanup.cleanup, Ok(()));
    let returned = DeviceExecution::retire_with_material_retained(&mut f.execution).unwrap();
    assert!(returned.material.into_parts().execution.cancelled);
    assert_eq!((f.effects.get(), f.waits.get()), effects);
}

#[test]
fn over_type_bound_pulse_input_preserves_denial_and_owner_custody() {
    use core::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    fn ready<T>(future: impl Future<Output = T>) -> T {
        let mut future = core::pin::pin!(future);
        match future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(result) => result,
            Poll::Pending => panic!("immediate refusal"),
        }
    }
    let mut f = fixture();
    let execution = f.execution.as_mut().unwrap();
    let request = k::scheduler::HostCallRequest {
        node: execution.storage.roles.pulse,
        call: k::HostCallId(0),
        request: k::RequestId(0),
        input: k::BoundedValueRef::new(
            k::ValueRef {
                slot: 0,
                generation: 0,
                byte_len: 1,
            },
            1,
        )
        .unwrap(),
    };
    for length in conduit_pulse_capture::PULSE_READ_CANONICAL_REQUEST_BYTES + 1..=1024 {
        let bytes = vec![0xff; length];
        let before = execution.native.pulse.inspection().clone();
        assert!(ready(execution.native.pulse.invoke(
            request.node,
            request.call,
            request.request,
            &bytes
        ))
        .is_err());
        let input = native_service::test_pulse_input(request, &bytes)
            .ok()
            .unwrap();
        let reply = ready(native_service::pulse(&mut execution.native.pulse, input));
        let failure = native_service::test_reply_failure(&reply).unwrap();
        assert_eq!(failure.code, k::FailureCode::HostCallDenied);
        assert_eq!(execution.native.pulse.inspection(), &before);
        assert_eq!(f.effects.get(), 0);
    }
    assert!(native_service::test_pulse_input(request, &[0; 1025]).is_err());
    assert_eq!(run(execution).exit, native_worker::Exit::Drained);
    assert_eq!(
        f.effects.get(),
        3,
        "invalid requests must not spend next request or capability budget"
    );
}
