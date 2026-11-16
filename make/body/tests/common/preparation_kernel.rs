//! One bounded request/outcome kernel fixture shared by preparation contracts.
use conduit_kernel::{scheduler::*, *};
pub type TestKernel<D> =
    FixedScheduler<D, FixedValueStore<8, 128>, FixedSignLog<128>, 1, 2, 1, 2, 1, 1, 1, 8>;
pub fn prepared_kernel<D: StepBack<1>>(back: D, binding: HostCallBinding) -> TestKernel<D> {
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
    let mut calls = FixedHostCallBindings::<1>::new(1);
    calls.install(NodeId(0), binding).unwrap();
    calls.seal().unwrap();
    let capacity = |slot_start| CordCapacity {
        slot_start,
        item_capacity: 1,
        byte_capacity: 128,
        pressure_policy: Default::default(),
    };
    let signs = FixedSignLog::<128>::new_with_remote_storage(
        128 * core::mem::size_of::<KernelEvent>() as u32,
        32,
        remote_sign_storage_bytes(32).unwrap(),
    )
    .unwrap();
    TestKernel::new_with_host_calls(
        [NodeSpec {
            input_cords: [Some(CordId(0))],
            maximum_step_fuel: 16,
        }],
        [
            CordSpec::remote_ingress(
                CordId(0),
                RemoteEndpointId(0),
                (NodeId(0), PortId(0)),
                capacity(0),
            ),
            CordSpec::remote_egress(
                CordId(1),
                (NodeId(0), PortId(0)),
                RemoteEndpointId(1),
                capacity(1),
            ),
        ],
        routes,
        calls,
        [back],
        FixedValueStore::new(1024).unwrap(),
        signs,
    )
    .unwrap()
}
