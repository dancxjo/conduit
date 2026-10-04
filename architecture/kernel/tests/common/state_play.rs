use conduit_kernel::scheduler::{
    CordCapacity, CordSpec, FixedScheduler, NodeSpec, SchedulerStatus,
};
use conduit_kernel::state_delay::back::StateBack;
use conduit_kernel::{
    CordEndpoint, CordId, FixedRoutes, FixedSignLog, FixedValueStore, KernelEvent, NodeId, PortId,
    RemoteEndpointId, RouteRange, RouteTarget,
};

pub type Play<const BYTES: usize> = FixedScheduler<
    StateBack<BYTES>,
    FixedValueStore<4, BYTES>,
    FixedSignLog<512>,
    1,
    2,
    1,
    2,
    1,
    1,
>;

pub fn with_back<const BYTES: usize>(back: StateBack<BYTES>) -> Play<BYTES> {
    let mut routes = FixedRoutes::<1, 1>::new(1);
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
    let signs = FixedSignLog::<512>::new_with_remote_storage(
        (512 * core::mem::size_of::<KernelEvent>()) as u32,
        512,
        conduit_kernel::remote_sign_storage_bytes(512).unwrap(),
    )
    .unwrap();
    Play::new(
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
                    item_capacity: 1,
                    byte_capacity: BYTES as u32,
                    pressure_policy: Default::default(),
                },
            ),
            CordSpec::remote_egress(
                CordId(1),
                (NodeId(0), PortId(0)),
                RemoteEndpointId(1),
                CordCapacity {
                    slot_start: 1,
                    item_capacity: 1,
                    byte_capacity: BYTES as u32,
                    pressure_policy: Default::default(),
                },
            ),
        ],
        routes,
        [back],
        FixedValueStore::<4, BYTES>::new((4 * BYTES) as u32).unwrap(),
        signs,
    )
    .unwrap()
}

pub fn idle<const BYTES: usize>(play: &mut Play<BYTES>) {
    for _ in 0..8 {
        match play.step().unwrap() {
            SchedulerStatus::Progress { .. } => {}
            SchedulerStatus::Idle => return,
            other => panic!("waiting is not terminal: {other:?}"),
        }
    }
    panic!("bounded test failed to reach input wait");
}

pub fn deliver<const BYTES: usize>(play: &mut Play<BYTES>, sequence: u64, expected: &[u8]) {
    let offer = play
        .remote_egress_offer(RemoteEndpointId(1), CordId(1))
        .unwrap()
        .unwrap();
    assert_eq!(offer.sequence, sequence);
    assert_eq!(play.host_value(offer.value).unwrap(), expected);
    play.remote_egress_accept(RemoteEndpointId(1), CordId(1), sequence)
        .unwrap();
    play.remote_egress_delivered(RemoteEndpointId(1), CordId(1), sequence)
        .unwrap();
}
