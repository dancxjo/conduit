use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use conduit_core::Plan;
use conduit_kernel::{
    scheduler::{FixedScheduler, OperationDriver, RemoteIngressOutcome, SchedulerStatus},
    FixedRoutes, FixedSignLog, FixedValueStore, Operation, OperationAction, OperationInput, PortId,
    RemoteEndpointId, SignSink, ValueRef, ValueStorage,
};
use conduit_runtime::lowering::{
    lower_plan_fragment, LoweredPlanFragment, RemoteCordDirection, MAXIMUM_KERNEL_PORTS_PER_NODE,
};

const PORTS: usize = MAXIMUM_KERNEL_PORTS_PER_NODE;
const VALUE_SLOTS: usize = 8;
const VALUE_BYTES: usize = 384;
const SIGNS: usize = 128;

#[derive(Clone)]
enum Leaf {
    Source { value: ValueRef, emitted: bool },
    Pass { emitted: bool },
    Sink { seen: Arc<AtomicBool> },
}

impl Operation for Leaf {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Source { value, .. } => OperationAction::Emit {
                port: PortId(0),
                value: *value,
            },
            Self::Pass { .. } | Self::Sink { .. } => OperationAction::Await,
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match (self, input) {
            (Self::Pass { emitted }, OperationInput::Value { value, .. }) if !*emitted => {
                *emitted = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value,
                }
            }
            (Self::Sink { seen }, OperationInput::Value { .. }) => {
                seen.store(true, Ordering::SeqCst);
                OperationAction::Complete
            }
            _ => OperationAction::Await,
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Source { emitted, .. } if !*emitted => {
                *emitted = true;
                OperationAction::Complete
            }
            Self::Pass { emitted: true } => OperationAction::Complete,
            _ => OperationAction::Await,
        }
    }

    fn cancel(&mut self) {}
}

type Driver = OperationDriver<Leaf, PORTS>;
type AKernel = FixedScheduler<
    Driver,
    FixedValueStore<VALUE_SLOTS, VALUE_BYTES>,
    FixedSignLog<SIGNS>,
    5,
    5,
    PORTS,
    5,
    { 5 * PORTS },
    5,
>;
type BKernel = FixedScheduler<
    Driver,
    FixedValueStore<VALUE_SLOTS, VALUE_BYTES>,
    FixedSignLog<SIGNS>,
    2,
    3,
    PORTS,
    3,
    { 2 * PORTS },
    3,
>;

pub(super) fn execute(plan: &Plan) {
    let part_a = plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == "part-a")
        .unwrap();
    let part_b = plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == "part-b")
        .unwrap();
    let lowered_a = lower_plan_fragment(part_a).unwrap();
    let lowered_b = lower_plan_fragment(part_b).unwrap();
    assert_eq!((lowered_a.nodes.len(), lowered_a.cords.len()), (5, 5));
    assert_eq!((lowered_b.nodes.len(), lowered_b.cords.len()), (2, 3));
    assert_eq!(lowered_a.remote_endpoints.len(), 2);
    assert_eq!(lowered_b.remote_endpoints.len(), 2);

    let sink_seen = Arc::new(AtomicBool::new(false));
    let (mut a, source_value) = kernel_a(part_a, &lowered_a, sink_seen.clone());
    let mut b = kernel_b(part_b, &lowered_b);
    let a_to_b = endpoint_pair(&lowered_a, &lowered_b, RemoteCordDirection::Egress);
    let b_to_a = endpoint_pair(&lowered_b, &lowered_a, RemoteCordDirection::Egress);
    let mut a_to_b_closed = false;
    let mut b_to_a_closed = false;
    let mut pressure_observed = false;
    let mut a_complete = false;
    let mut b_complete = false;

    for _turn in 0..128 {
        a_complete |= matches!(a.step().unwrap(), SchedulerStatus::Complete);
        b_complete |= matches!(b.step().unwrap(), SchedulerStatus::Complete);

        if let Some(offer) = a.remote_egress_offer(a_to_b.0, a_to_b.1).unwrap() {
            if !pressure_observed {
                assert_eq!(
                    a.remote_egress_offer(a_to_b.0, a_to_b.1).unwrap(),
                    Some(offer)
                );
                assert!(a.values().reference_count(source_value).unwrap() > 0);
                pressure_observed = true;
            } else {
                transfer_a_to_b(&mut a, &mut b, a_to_b, offer.sequence, offer.value);
            }
        } else if !a_to_b_closed && a.remote_egress_terminal(a_to_b.0, a_to_b.1).unwrap() {
            b.close_remote_input(a_to_b.2, a_to_b.3).unwrap();
            a_to_b_closed = true;
        }

        if let Some(offer) = b.remote_egress_offer(b_to_a.0, b_to_a.1).unwrap() {
            transfer_b_to_a(&mut b, &mut a, b_to_a, offer.sequence, offer.value);
        } else if !b_to_a_closed && b.remote_egress_terminal(b_to_a.0, b_to_a.1).unwrap() {
            a.close_remote_input(b_to_a.2, b_to_a.3).unwrap();
            b_to_a_closed = true;
        }
        if a_complete && b_complete {
            break;
        }
    }

    assert!(pressure_observed);
    assert!(a_complete && b_complete);
    assert!(sink_seen.load(Ordering::SeqCst));
    assert!(a_to_b_closed && b_to_a_closed);
    assert!(usize::from(a.signs().len()) <= SIGNS && usize::from(b.signs().len()) <= SIGNS);

    let cancelled_seen = Arc::new(AtomicBool::new(false));
    let (mut cancelled_a, _) = kernel_a(part_a, &lowered_a, cancelled_seen);
    let mut cancelled_b = kernel_b(part_b, &lowered_b);
    let pending = loop {
        cancelled_a.step().unwrap();
        cancelled_b.step().unwrap();
        if let Some(offer) = cancelled_a.remote_egress_offer(a_to_b.0, a_to_b.1).unwrap() {
            break offer;
        }
    };
    cancelled_a.cancel().unwrap();
    cancelled_b.cancel().unwrap();
    assert_eq!(cancelled_a.step().unwrap(), SchedulerStatus::Cancelled);
    assert_eq!(cancelled_b.step().unwrap(), SchedulerStatus::Cancelled);
    assert!(cancelled_a
        .remote_egress_accept(a_to_b.0, a_to_b.1, pending.sequence)
        .is_err());
    assert!(cancelled_b
        .admit_remote_input(a_to_b.2, a_to_b.3, pending.sequence, b"late")
        .is_err());
}

fn kernel_a(
    fragment: &conduit_core::PlanFragment,
    lowered: &LoweredPlanFragment,
    seen: Arc<AtomicBool>,
) -> (AKernel, ValueRef) {
    let mut values = FixedValueStore::<VALUE_SLOTS, VALUE_BYTES>::new(VALUE_BYTES as u32).unwrap();
    let source = values.store(b"provider-prompt").unwrap();
    let drivers = core::array::from_fn(|index| {
        let kind = fragment.placements[index].kind_id.as_str();
        let leaf = if kind == super::SOURCE {
            Leaf::Source {
                value: source,
                emitted: false,
            }
        } else if kind == super::SINK {
            Leaf::Sink { seen: seen.clone() }
        } else {
            Leaf::Pass { emitted: false }
        };
        OperationDriver::new(leaf).unwrap()
    });
    let kernel = FixedScheduler::new(
        lowered.node_specs.clone().try_into().unwrap(),
        lowered
            .cords
            .iter()
            .map(|cord| cord.spec)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap(),
        routes::<{ 5 * PORTS }, 5>(lowered),
        drivers,
        values,
        FixedSignLog::new((SIGNS * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32)
            .unwrap(),
    )
    .unwrap();
    (kernel, source)
}

fn kernel_b(fragment: &conduit_core::PlanFragment, lowered: &LoweredPlanFragment) -> BKernel {
    let drivers =
        core::array::from_fn(|_| OperationDriver::new(Leaf::Pass { emitted: false }).unwrap());
    assert!(fragment
        .placements
        .iter()
        .all(|placement| matches!(placement.kind_id.as_str(), super::HTTP | super::DECODE)));
    FixedScheduler::new(
        lowered.node_specs.clone().try_into().unwrap(),
        lowered
            .cords
            .iter()
            .map(|cord| cord.spec)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap(),
        routes::<{ 2 * PORTS }, 3>(lowered),
        drivers,
        FixedValueStore::<VALUE_SLOTS, VALUE_BYTES>::new(VALUE_BYTES as u32).unwrap(),
        FixedSignLog::new((SIGNS * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32)
            .unwrap(),
    )
    .unwrap()
}

fn routes<const SLOTS: usize, const TARGETS: usize>(
    lowered: &LoweredPlanFragment,
) -> FixedRoutes<SLOTS, TARGETS> {
    let mut routes = FixedRoutes::new(PORTS as u16);
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
    routes
}

fn endpoint_pair(
    source: &LoweredPlanFragment,
    sink: &LoweredPlanFragment,
    direction: RemoteCordDirection,
) -> (
    RemoteEndpointId,
    conduit_kernel::CordId,
    RemoteEndpointId,
    conduit_kernel::CordId,
) {
    let egress = source
        .remote_endpoints
        .iter()
        .find(|endpoint| endpoint.direction == direction)
        .unwrap();
    let ingress = sink
        .remote_endpoints
        .iter()
        .find(|endpoint| {
            endpoint.direction == RemoteCordDirection::Ingress
                && endpoint.connection_id == egress.connection_id
        })
        .unwrap();
    (egress.endpoint, egress.cord, ingress.endpoint, ingress.cord)
}

fn transfer_a_to_b(
    source: &mut AKernel,
    sink: &mut BKernel,
    endpoints: (
        RemoteEndpointId,
        conduit_kernel::CordId,
        RemoteEndpointId,
        conduit_kernel::CordId,
    ),
    sequence: u64,
    value: ValueRef,
) {
    let bytes = source.host_value(value).unwrap().to_vec();
    assert_eq!(
        sink.admit_remote_input(endpoints.2, endpoints.3, sequence, &bytes)
            .unwrap(),
        RemoteIngressOutcome::Accepted { sequence }
    );
    source
        .remote_egress_accept(endpoints.0, endpoints.1, sequence)
        .unwrap();
    source
        .remote_egress_delivered(endpoints.0, endpoints.1, sequence)
        .unwrap();
}

fn transfer_b_to_a(
    source: &mut BKernel,
    sink: &mut AKernel,
    endpoints: (
        RemoteEndpointId,
        conduit_kernel::CordId,
        RemoteEndpointId,
        conduit_kernel::CordId,
    ),
    sequence: u64,
    value: ValueRef,
) {
    let bytes = source.host_value(value).unwrap().to_vec();
    assert_eq!(
        sink.admit_remote_input(endpoints.2, endpoints.3, sequence, &bytes)
            .unwrap(),
        RemoteIngressOutcome::Accepted { sequence }
    );
    source
        .remote_egress_accept(endpoints.0, endpoints.1, sequence)
        .unwrap();
    source
        .remote_egress_delivered(endpoints.0, endpoints.1, sequence)
        .unwrap();
}
