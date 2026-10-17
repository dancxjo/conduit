//! Preparation route traversal and exact, globally single-source membership.
use crate::{FixedRoutes, NodeId, PortId, ProtocolError, RouteTarget};

pub(super) fn targets<const SLOTS: usize, const TARGETS: usize>(
    routes: &FixedRoutes<SLOTS, TARGETS>,
    node: NodeId,
    port: PortId,
) -> Result<impl Iterator<Item = RouteTarget> + '_, ProtocolError> {
    let targets = routes.route(node, port)?;
    #[cfg(test)]
    let targets = targets.inspect(|_| TARGET_VISITS.with(|count| count.set(count.get() + 1)));
    Ok(targets)
}

pub(super) struct RouteMembership<const CORDS: usize> {
    seen: [bool; CORDS],
}
impl<const CORDS: usize> RouteMembership<CORDS> {
    pub(super) fn new() -> Self {
        #[cfg(test)]
        INITIALIZED_BYTES.with(|count| count.set(count.get() + CORDS));
        Self {
            seen: [false; CORDS],
        }
    }
    pub(super) fn contains(&self, cord: usize) -> bool {
        self.seen[cord]
    }
    pub(super) fn mark(&mut self, cord: usize) {
        self.seen[cord] = true;
    }
}

#[cfg(test)]
extern crate std;
#[cfg(test)]
std::thread_local! {
    static INITIALIZED_BYTES: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
    static TARGET_VISITS: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::{
        CordCapacity, CordSpec, FixedScheduler, NodeSpec, SchedulerError, StepBack, StepInputBytes,
        StepIo, StepOutcome,
    };
    use crate::{
        CordEndpoint, CordId, FixedRoutes, FixedSignLog, FixedValueStore, NodeId, PortId,
        RemoteEndpointId, RouteRange, RouteTarget,
    };
    const N: usize = 256;
    #[derive(Clone, Copy)]
    struct Await;
    impl StepBack<1> for Await {
        fn step(&mut self, _: &mut StepIo<1>, _: &StepInputBytes<'_, 1>) -> StepOutcome {
            StepOutcome::Await
        }
    }
    type Scheduler =
        FixedScheduler<Await, FixedValueStore<1, 1>, FixedSignLog<16>, N, N, 1, N, N, 512, N, 1>;
    fn parts(
        remote: bool,
        duplicate: bool,
        wrong_source: bool,
    ) -> ([NodeSpec<1>; N], [CordSpec; N], FixedRoutes<N, 512>) {
        let nodes = core::array::from_fn(|node| NodeSpec {
            input_cords: [Some(CordId(
                if remote { node } else { (node + N - 1) % N } as u16
            ))],
            maximum_step_fuel: 8,
        });
        let cords = core::array::from_fn(|node| {
            let capacity = CordCapacity {
                slot_start: node as u16,
                item_capacity: 1,
                byte_capacity: 1,
                pressure_policy: Default::default(),
            };
            if remote {
                CordSpec::remote_ingress(
                    CordId(node as u16),
                    RemoteEndpointId(node as u16),
                    (NodeId(node as u16), PortId(0)),
                    capacity,
                )
            } else {
                CordSpec::local(
                    CordId(node as u16),
                    (NodeId(node as u16), PortId(0)),
                    (NodeId(((node + 1) % N) as u16), PortId(0)),
                    capacity,
                )
            }
        });
        let mut routes = FixedRoutes::new(1);
        if !remote {
            for node in 0..N {
                let cord = if wrong_source && node == 1 { 0 } else { node };
                let target = RouteTarget {
                    cord: CordId(cord as u16),
                    sink: CordEndpoint::local(NodeId(((cord + 1) % N) as u16), PortId(0)),
                };
                let targets = [target, target];
                let len = if duplicate && node == 0 { 2 } else { 1 };
                routes
                    .install(
                        NodeId(node as u16),
                        PortId(0),
                        RouteRange {
                            start: (node * 2) as u16,
                            len,
                        },
                        &targets[..usize::from(len)],
                    )
                    .unwrap();
            }
        }
        routes.seal().unwrap();
        (nodes, cords, routes)
    }
    fn prepare(
        remote: bool,
        duplicate: bool,
        wrong_source: bool,
    ) -> Result<Scheduler, SchedulerError> {
        let (nodes, cords, routes) = parts(remote, duplicate, wrong_source);
        INITIALIZED_BYTES.with(|count| count.set(0));
        Scheduler::new(
            nodes,
            cords,
            routes,
            [Await; N],
            FixedValueStore::new(1).unwrap(),
            FixedSignLog::new(16 * core::mem::size_of::<crate::KernelEvent>() as u32).unwrap(),
        )
    }
    #[test]
    fn routed_preparation_initializes_cord_membership_once() {
        assert!(prepare(false, false, false).is_ok());
        assert_eq!(INITIALIZED_BYTES.with(|count| count.get()), N);
    }
    #[test]
    fn ingress_only_preparation_needs_no_route_membership_scratch() {
        assert!(prepare(true, false, false).is_ok());
        assert_eq!(INITIALIZED_BYTES.with(|count| count.get()), 0);
    }
    #[test]
    fn duplicate_and_cross_source_route_targets_remain_invalid() {
        for (duplicate, wrong_source) in [(true, false), (false, true)] {
            assert_eq!(
                prepare(false, duplicate, wrong_source).err(),
                Some(SchedulerError::InvalidPlan)
            );
        }
    }
    #[cfg(feature = "alloc")]
    #[test]
    fn boxed_preparation_initializes_cord_membership_once() {
        let (nodes, cords, routes) = parts(false, false, false);
        let mut bindings = crate::FixedHostCallBindings::new(1);
        bindings.seal().unwrap();
        INITIALIZED_BYTES.with(|count| count.set(0));
        assert!(Scheduler::new_boxed_with_active_counts_and_host_calls(
            N,
            N,
            nodes,
            cords,
            routes,
            bindings,
            [Await; N],
            FixedValueStore::new(1).unwrap(),
            FixedSignLog::new(16 * core::mem::size_of::<crate::KernelEvent>() as u32).unwrap()
        )
        .is_ok());
        assert_eq!(INITIALIZED_BYTES.with(|count| count.get()), N);
    }
    fn fanout(limit: Option<usize>) -> Result<(), SchedulerError> {
        let mut routes = FixedRoutes::<1, N>::new(1);
        let targets = core::array::from_fn::<_, N, _>(|cord| RouteTarget {
            cord: CordId(cord as u16),
            sink: CordEndpoint::Remote(RemoteEndpointId(cord as u16)),
        });
        if let Some(limit) = limit {
            routes
                .install(
                    NodeId(0),
                    PortId(0),
                    RouteRange {
                        start: 0,
                        len: limit as u16,
                    },
                    &targets[..limit],
                )
                .unwrap();
        }
        routes.seal().unwrap();
        let cords = core::array::from_fn(|cord| {
            CordSpec::remote_egress(
                CordId(cord as u16),
                (NodeId(0), PortId(0)),
                RemoteEndpointId(cord as u16),
                CordCapacity {
                    slot_start: cord as u16,
                    item_capacity: 1,
                    byte_capacity: 1,
                    pressure_policy: Default::default(),
                },
            )
        });
        TARGET_VISITS.with(|count| count.set(0));
        FixedScheduler::<_, _, _, 1, N, 1, N, 1, N>::new(
            [NodeSpec {
                input_cords: [None],
                maximum_step_fuel: 8,
            }],
            cords,
            routes,
            [Await],
            FixedValueStore::<1, 1>::new(1).unwrap(),
            FixedSignLog::<16>::new(16 * core::mem::size_of::<crate::KernelEvent>() as u32)
                .unwrap(),
        )
        .map(|_| ())
    }

    #[test]
    fn source_fanout_targets_are_validated_once_without_reverse_walks() {
        assert_eq!(fanout(Some(N)), Ok(()));
        assert_eq!(TARGET_VISITS.with(|count| count.get()), N);
    }

    #[test]
    fn missing_empty_and_incomplete_routes_preserve_exact_refusals() {
        assert_eq!(
            fanout(None),
            Err(SchedulerError::Routing(ProtocolError::RouteMissing))
        );
        for limit in [0, N - 1] {
            assert_eq!(fanout(Some(limit)), Err(SchedulerError::InvalidPlan));
        }
    }
}
