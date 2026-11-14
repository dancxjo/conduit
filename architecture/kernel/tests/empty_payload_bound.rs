//! Unit-valued Cords have zero canonical bytes but still consume queue items.
use conduit_kernel::{scheduler::*, *};
struct Sink;
impl StepBack<1> for Sink {
    fn step(&mut self, _: &mut StepIo<1>, _: &StepInputBytes<'_, 1>) -> StepOutcome {
        StepOutcome::Await
    }
}
#[test]
fn zero_semantic_bound_accepts_empty_payload_and_rejects_bytes() {
    type Kernel = FixedScheduler<Sink, FixedValueStore<2, 1>, FixedSignLog<32>, 1, 1, 1, 1, 1, 0>;
    let mut routes = FixedRoutes::<1, 0>::new(1);
    routes.seal().unwrap();
    let cord = CordSpec::remote_ingress(
        CordId(0),
        RemoteEndpointId(0),
        (NodeId(0), PortId(0)),
        CordCapacity {
            slot_start: 0,
            item_capacity: 1,
            byte_capacity: 1,
            pressure_policy: Default::default(),
        },
    )
    .with_maximum_value_bytes(0);
    let mut kernel = Kernel::new(
        [NodeSpec {
            input_cords: [Some(CordId(0))],
            maximum_step_fuel: 1,
        }],
        [cord],
        routes,
        [Sink],
        FixedValueStore::new(2).unwrap(),
        FixedSignLog::new_with_remote_storage(
            (32 * core::mem::size_of::<KernelEvent>()) as u32,
            2,
            remote_sign_storage_bytes(2).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        kernel.admit_remote_input(RemoteEndpointId(0), CordId(0), 0, &[1]),
        Err(SchedulerError::SemanticValueBoundExceeded)
    );
    assert!(matches!(
        kernel
            .admit_remote_input(RemoteEndpointId(0), CordId(0), 0, &[])
            .unwrap(),
        RemoteIngressOutcome::Accepted { .. }
    ));
    assert!(matches!(
        kernel
            .admit_remote_input(RemoteEndpointId(0), CordId(0), 1, &[])
            .unwrap(),
        RemoteIngressOutcome::Full { .. }
    ));
}
