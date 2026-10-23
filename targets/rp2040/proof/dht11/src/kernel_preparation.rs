//! Bounded preparation of the bench Backs, storage, native identity and kernel.
extern crate alloc;
use super::{Back, BenchKernel, Observed, Role, C, N, P, R, ROLES};
use alloc::boxed::Box;
use conduit_core as c;
use conduit_environment_runtime::{CaptureBack, PreparedCaptureSelector};
use conduit_kernel as k;
use conduit_plot::PortableExpressionProgram;
use conduit_pulse_capture::installation::PulseNativeIdentity;
pub(super) fn identity(boot: &str) -> PulseNativeIdentity {
    PulseNativeIdentity {
        host_id: "fixture/environment".into(),
        boot_id: boot.into(),
        base_id: "fixture/pulse-base".into(),
        provider_instance_id: "fixture/pulse-provider".into(),
        provider_generation: 1,
        resource_pool_id: "fixture/pulse-resource".into(),
        resource_generation_id: c::ResourceGenerationId("fixture/attachment/1".into()),
        envelope_id: "fixture/pulse-envelope".into(),
        artifact_id: "fixture/pulse-artifact".into(),
        implementation_id: "fixture/pulse-read@1".into(),
        base_implementation_id: "fixture/pulse-base@1".into(),
    }
}

#[inline(never)]
pub(super) fn prepare_back<'a>(
    role: Role,
    observed: &'a [Observed],
    selector: &mut Option<PreparedCaptureSelector>,
    wait: k::ValueRef,
) -> Back<'a> {
    match role {
        Role::Trigger => Back::Trigger(conduit_time::TickTriggerBack::default()),
        Role::Tick(count, _) => Back::Tick(conduit_time::TickBack::counted(wait, count).unwrap()),
        Role::Begin => Back::Begin {
            sent: false,
            staged: false,
        },
        Role::Observe(index) => Back::Observe {
            observed: &observed[index],
            pending: alloc::vec![0; observed[index].bytes.borrow().0.len()].into_boxed_slice(),
            length: 0,
            staged: false,
        },
        Role::Observation => Back::Native(
            CaptureBack::observation(
                &conduit_environment_runtime::observation::ObservationContract::prepare().unwrap(),
            )
            .unwrap(),
        ),
        Role::Pulse => Back::Native(CaptureBack::pulse(1024)),
        Role::Expression(bytes) => Back::Native(
            CaptureBack::expression(
                &PortableExpressionProgram::from_canonical_bytes(bytes).unwrap(),
                c::PortTemporal::Flow { closes: true },
            )
            .unwrap(),
        ),
        Role::Selector(text) => {
            *selector = Some(
                PreparedCaptureSelector::prepare(
                    c::StructuredSelector::from_canonical_hex(text).unwrap(),
                    1024,
                    1024,
                )
                .unwrap(),
            );
            Back::Native(CaptureBack::selector(1024))
        }
    }
}

#[inline(never)]
pub(super) fn construct_kernel<'a>(
    nodes: [k::scheduler::NodeSpec<P>; N],
    cords: [k::scheduler::CordSpec; C],
    routes: k::FixedRoutes<R, C>,
    calls: k::FixedHostCallBindings<N>,
    backs: Box<[Option<Back<'a>>; N]>,
    signs: k::HostedSignLog,
    values: k::HostedValueStore,
) -> Box<BenchKernel<'a>> {
    let backs = (*backs).map(Option::unwrap);
    Box::new(
        BenchKernel::new_with_host_calls(nodes, cords, routes, calls, backs, values, signs)
            .unwrap(),
    )
}

#[inline(never)]
pub(super) fn prepare_values() -> (k::HostedValueStore, k::ValueRef) {
    use k::ValueStorage;
    let period = ROLES
        .iter()
        .find_map(|r| {
            if let Role::Tick(_, period) = r {
                Some(*period)
            } else {
                None
            }
        })
        .unwrap();
    let mut capacities = [1024; 16];
    capacities[..2].fill(4096);
    let mut values = k::HostedValueStore::new_with_slot_capacities(&capacities, 22528).unwrap();
    let wait = values.store(&period.to_le_bytes()).unwrap();
    (values, wait)
}

// These witnesses contain full canonical buffers. Constructing them outside the
// async poll keeps their large move temporaries off later Back-preparation calls.
#[inline(never)]
pub(super) fn observation_witnesses() -> alloc::vec::Vec<Observed> {
    (0..4)
        .map(|index| {
            let capacity = if cfg!(feature = "http-ap") && index > 0 {
                1024
            } else {
                4096
            };
            Observed::new(capacity)
        })
        .collect()
}
#[inline(never)]
pub(super) fn vacant_backs<'a>() -> Box<[Option<Back<'a>>; N]> {
    Box::new(core::array::from_fn(|_| None))
}

// Native contract preparation has large temporary witnesses. Keep them off
// Root's poll stack while the independently admitted network service runs.
#[inline(never)]
pub(super) fn bind_owner<T: conduit_pulse_capture::owner::PulseReadProvider>(
    ready: conduit_pulse_capture::installation::ReadyPulseBase<T>,
    identity: &PulseNativeIdentity,
    plan_id: &c::PlanId,
    active: &c::ActivePlayIdentity,
    selection: conduit_plan_lowering::selected_operation::SelectedOperationPlan<'_>,
) -> conduit_pulse_capture::owner::PulseHostCall<T> {
    let (table, handle, claim) = crate::kernel_authority::possession(identity, plan_id, active);
    ready
        .bind_selected(
            table,
            handle,
            claim,
            &conduit_pulse_capture::PulseReadContract::prepare().unwrap(),
            selection,
        )
        .unwrap()
}
