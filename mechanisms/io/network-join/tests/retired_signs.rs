//! Scheduling storage retirement only; this fixture has no native or Body authority.
use conduit_kernel::{
    self as k,
    scheduler::{CordCapacity, CordSpec, NodeSpec},
    ValueStorage,
};
use conduit_network_join_call::{
    NetworkControlBack, NetworkControlKernel, NetworkControlRoles, PreparedNetworkKernel,
};
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn prepared() -> Box<PreparedNetworkKernel> {
    let mut values = k::FixedValueStore::<8, 167>::new(8 * 167).unwrap();
    let empty = values.store(&[]).unwrap();
    let mut routes = k::FixedRoutes::<3, 2>::new(1);
    for id in 0..2 {
        routes
            .install(
                k::NodeId(id),
                k::PortId(0),
                k::RouteRange { start: id, len: 1 },
                &[k::RouteTarget {
                    cord: k::CordId(id),
                    sink: k::CordEndpoint::local(k::NodeId(id + 1), k::PortId(0)),
                }],
            )
            .unwrap();
    }
    routes.seal().unwrap();
    let mut bindings = k::FixedHostCallBindings::<3>::new(1);
    for id in 0..3 {
        bindings
            .install(
                k::NodeId(id),
                k::HostCallBinding {
                    call: k::HostCallId(0),
                    maximum_input_bytes: if id == 0 { 1 } else { 167 },
                    maximum_output_bytes: if id == 2 { 1 } else { 167 },
                },
            )
            .unwrap();
    }
    bindings.seal().unwrap();
    let nodes = core::array::from_fn(|i| NodeSpec {
        input_cords: [if i == 0 {
            None
        } else {
            Some(k::CordId(i as u16 - 1))
        }],
        maximum_step_fuel: 4,
    });
    let cords = core::array::from_fn(|i| {
        CordSpec::local(
            k::CordId(i as u16),
            (k::NodeId(i as u16), k::PortId(0)),
            (k::NodeId(i as u16 + 1), k::PortId(0)),
            CordCapacity {
                slot_start: i as u16,
                item_capacity: 1,
                byte_capacity: 167,
                pressure_policy: Default::default(),
            },
        )
    });
    let kernel = NetworkControlKernel::new_with_host_calls(
        nodes,
        cords,
        routes,
        bindings,
        [
            NetworkControlBack::credentials(empty).unwrap(),
            NetworkControlBack::join(),
            NetworkControlBack::attachment_sign(),
        ],
        values,
        k::HostedSignLog::new(128, (128 * core::mem::size_of::<k::KernelEvent>()) as u32).unwrap(),
    )
    .unwrap();
    Box::new(PreparedNetworkKernel {
        kernel,
        roles: NetworkControlRoles {
            credentials: k::NodeId(0),
            join: k::NodeId(1),
            attachment_sign: k::NodeId(2),
        },
    })
}
#[test]
fn active_refusal_preserves_the_box_and_terminal_retirement_keeps_exact_signs() {
    let owner = prepared();
    let pointer = &*owner as *const PreparedNetworkKernel;
    let (owner, memory) = allocation_probe::observe(|| match owner.try_retire_signs() {
        Ok(_) => panic!("active kernel retired"),
        Err(owner) => owner,
    });
    assert_eq!(pointer, &*owner as *const PreparedNetworkKernel);
    assert_eq!(
        memory.allocations + memory.reallocations + memory.deallocations,
        0
    );
    let mut owner = owner;
    owner.kernel.cancel().unwrap();
    let expected = owner.kernel.signs().events().collect::<Vec<_>>();
    let (signs, memory) = allocation_probe::observe(|| {
        owner
            .try_retire_signs()
            .unwrap_or_else(|_| panic!("cancelled kernel refused"))
    });
    assert_eq!(memory.allocations + memory.reallocations, 0);
    assert!(memory.deallocated_bytes >= core::mem::size_of::<PreparedNetworkKernel>());
    assert_eq!(signs.events().collect::<Vec<_>>(), expected);
    assert_eq!(signs.allocation_capacity(), 128);
}
