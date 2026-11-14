//! CPU geometry/custody proof only: no Body approval or native effect authority.
use super::*;
use crate::{
    execution_workset::PreparedLocalKernels,
    member_return_tests::return_allocation_probe as allocation,
};
use core::cell::Cell;

struct Echo;
impl StepBack<1> for Echo {
    fn step(&mut self, io: &mut StepIo<1>, _: &StepInputBytes<'_, 1>) -> StepOutcome {
        if io.input(PortId(0)).is_none() || !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let value = io.take_input(PortId(0)).unwrap();
        io.send(PortId(0), value).unwrap();
        StepOutcome::Progress
    }
}
struct Backs<'a>(&'a Cell<usize>);
// SAFETY: one inert CPU Back, exact no-terminal report, no native owners or
// allocation/effects. Binding merely counts transfer into validated storage.
unsafe impl PreparedBacks<1, 1> for Backs<'_> {
    type Driver = Echo;
    type Calls = ();
    fn retained_backing_bytes(&self) -> Option<usize> {
        Some(0)
    }
    #[cfg(feature = "execution-workspace")]
    fn execution_workspace_requirements(&self) -> Option<[Option<StepWorkspaceRequirements>; 1]> {
        Some([None])
    }
    fn bind(self) -> ([Echo; 1], ()) {
        self.0.set(self.0.get() + 1);
        ([Echo], ())
    }
}
type Kernel<const Q: usize> =
    ExecutionKernel<Echo, 1, 2, 1, 1, 1, 1, 64, FixedSignLog<64>, 1, HostedValueStore, Q>;
type Fragment<'a, const Q: usize> =
    PreparedFragment<Backs<'a>, 1, 2, 1, 1, 1, 1, 64, FixedSignLog<64>, 1, HostedValueStore, Q>;
fn prepare<const Q: usize>(
    binds: &Cell<usize>,
    maximum: usize,
) -> Result<Fragment<'_, Q>, KernelPreparationError> {
    let mut routes = FixedRoutes::new(1);
    routes
        .install(
            NodeId(0),
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(1),
                sink: CordEndpoint::Remote(RemoteEndpointId(1)),
            }],
        )
        .unwrap();
    routes.seal().unwrap();
    let mut calls = FixedHostCallBindings::new(1);
    calls.seal().unwrap();
    let values = HostedValueStore::new(8, 8, 64).unwrap();
    let backing = values.reserved_storage_bytes();
    let signs = FixedSignLog::<64>::new_with_remote_storage(
        (64 * core::mem::size_of::<KernelEvent>()) as u32,
        64,
        remote_sign_storage_bytes(64).unwrap(),
    )
    .unwrap();
    let storage = Kernel::<Q>::prepare_boxed(
        1,
        2,
        [NodeSpec {
            input_cords: [Some(CordId(0))],
            maximum_step_fuel: 4,
        }],
        [
            CordSpec::remote_ingress(
                CordId(0),
                RemoteEndpointId(0),
                (NodeId(0), PortId(0)),
                CordCapacity {
                    slot_start: 0,
                    item_capacity: 2,
                    byte_capacity: 2,
                    pressure_policy: Default::default(),
                },
            ),
            CordSpec::remote_egress(
                CordId(1),
                (NodeId(0), PortId(0)),
                RemoteEndpointId(1),
                CordCapacity {
                    slot_start: 2,
                    item_capacity: 1,
                    byte_capacity: 1,
                    pressure_policy: Default::default(),
                },
            ),
        ],
        routes,
        calls,
        values,
        signs,
        maximum,
    )?;
    let retained = backing + storage.storage_bytes();
    Ok(PreparedFragment {
        storage,
        backs: Backs(binds),
        retained,
        #[cfg(feature = "execution-workspace")]
        maximum_retained_bytes: retained,
    })
}
#[test]
fn exact_queue_geometry_refuses_short_storage_before_back_transfer() {
    let binds = Cell::new(0);
    assert!(matches!(
        prepare::<2>(&binds, usize::MAX),
        Err(KernelPreparationError::Topology(
            SchedulerError::InvalidPlan
        ))
    ));
    assert_eq!(binds.get(), 0);
    let bytes = core::mem::size_of::<Kernel<3>>();
    assert!(bytes > core::mem::size_of::<Kernel<2>>());
    assert!(
        matches!(prepare::<3>(&binds,bytes-1),Err(KernelPreparationError::Capacity { required,available }) if required==bytes && available==bytes-1)
    );
    assert_eq!(binds.get(), 0);
    let prepared = prepare::<3>(&binds, bytes).unwrap();
    assert_eq!(prepared.storage.storage_bytes(), bytes);
    assert_eq!(
        prepared.retained,
        bytes
            + HostedValueStore::new(8, 8, 64)
                .unwrap()
                .reserved_storage_bytes()
    );
    assert_eq!(
        PreparedLocalKernels::retained_backing_bytes(&prepared),
        Some(prepared.retained)
    );
    drop(prepared);
    assert_eq!(binds.get(), 0);
}
#[test]
fn independent_queue_slots_preserve_step_pressure_order_evidence_and_no_allocations() {
    let binds = Cell::new(0);
    let mut prepared = prepare::<3>(&binds, usize::MAX).unwrap();
    #[cfg(feature = "execution-workspace")]
    {
        prepared.prepare_execution_workspace().unwrap();
    }
    // SAFETY: inert CPU owners and already validated storage. This checks
    // physical transfer; it is not a lawful Body admission or Start fixture.
    let ((mut kernel, ()), bind_cost) =
        allocation::observe(|| unsafe { PreparedLocalKernels::bind(prepared) });
    assert_eq!(binds.get(), 1);
    assert_eq!((bind_cost.allocations, bind_cost.reallocations), (0, 0));
    let (_, cost) = allocation::observe(|| {
        for (sequence, value) in [(0, 11), (1, 22)] {
            assert_eq!(
                kernel
                    .admit_remote_input(RemoteEndpointId(0), CordId(0), sequence, &[value])
                    .unwrap(),
                RemoteIngressOutcome::Accepted { sequence }
            );
        }
        assert_eq!(kernel.cord_usage(CordId(0)).unwrap(), (2, 2));
        assert_eq!(
            kernel
                .admit_remote_input(RemoteEndpointId(0), CordId(0), 2, &[33])
                .unwrap(),
            RemoteIngressOutcome::Full { sequence: 2 }
        );
        assert!(matches!(
            kernel.step().unwrap(),
            SchedulerStatus::Progress { .. }
        ));
        assert_eq!(kernel.cord_usage(CordId(0)).unwrap(), (1, 1));
        // The first blocked poll records its pressure transition. Repeated
        // pressure then becomes Idle without consuming the queued reading.
        assert!(matches!(
            kernel.step().unwrap(),
            SchedulerStatus::Progress { .. }
        ));
        assert_eq!(kernel.step().unwrap(), SchedulerStatus::Idle);
        assert_eq!(kernel.cord_usage(CordId(0)).unwrap(), (1, 1));
        for (sequence, value) in [(0, 11), (1, 22)] {
            let offer = kernel
                .remote_egress_offer(RemoteEndpointId(1), CordId(1))
                .unwrap()
                .unwrap();
            assert_eq!(offer.sequence, sequence);
            assert_eq!(kernel.host_value(offer.value).unwrap(), &[value]);
            assert_eq!(
                kernel
                    .remote_egress_offer(RemoteEndpointId(1), CordId(1))
                    .unwrap(),
                Some(offer)
            );
            kernel
                .remote_egress_accept(RemoteEndpointId(1), CordId(1), sequence)
                .unwrap();
            kernel
                .remote_egress_delivered(RemoteEndpointId(1), CordId(1), sequence)
                .unwrap();
            if sequence == 0 {
                assert!(matches!(
                    kernel.step().unwrap(),
                    SchedulerStatus::Progress { .. }
                ));
            }
        }
        // The pressure refusal never consumed sequence2 or discarded its value.
        assert_eq!(
            kernel
                .admit_remote_input(RemoteEndpointId(0), CordId(0), 2, &[33])
                .unwrap(),
            RemoteIngressOutcome::Accepted { sequence: 2 }
        );
        assert!(matches!(
            kernel.step().unwrap(),
            SchedulerStatus::Progress { .. }
        ));
    });
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    assert_eq!(
        kernel
            .signs()
            .events()
            .filter(|e| e.kind == KernelEventKind::RemoteInputAdmitted)
            .count(),
        3
    );
}
