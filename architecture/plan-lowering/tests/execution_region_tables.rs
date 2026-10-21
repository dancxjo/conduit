use conduit_kernel::{
    scheduler::{FixedScheduler, StepBack, StepInputBytes, StepIo, StepOutcome},
    CanonicalValue, CordEndpoint, CordId, FixedRoutes, FixedSignLog, FixedValueStore, NodeId,
    PortId, RemoteEndpointId, ValueStorage,
};
use conduit_plan_lowering::{
    execution_regions::*, lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE as PORTS,
};
#[path = "common/execution_regions.rs"]
mod fixtures;

#[derive(Clone, Copy)]
struct Back {
    source: bool,
    sent: bool,
    received: u8,
}
impl StepBack<PORTS> for Back {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.source {
            if self.sent || !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            io.send_canonical(PortId(0), CanonicalValue::new(&[11]).unwrap())
                .unwrap();
        } else {
            let Some(value) = bytes.input(PortId(0)) else {
                return StepOutcome::Await;
            };
            self.received = value[0];
            io.consume(PortId(0)).unwrap();
        }
        StepOutcome::Progress
    }
    fn step_committed(&mut self) {
        if self.source {
            self.sent = true;
        }
    }
}
type Scheduler =
    FixedScheduler<Back, FixedValueStore<2, 2>, FixedSignLog<64>, 1, 1, PORTS, 2, PORTS, 1>;
fn install(tables: &PreparedRegionKernelTables<'_, '_>, source: bool) -> Scheduler {
    let mut routes = FixedRoutes::<PORTS, 1>::new(PORTS as u16);
    for route in tables.routes() {
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
    FixedScheduler::new(
        [tables.nodes()[0]],
        [tables.cords()[0]],
        routes,
        [Back {
            source,
            sent: false,
            received: 0,
        }],
        FixedValueStore::new(2).unwrap(),
        FixedSignLog::new_with_remote_storage(
            64 * core::mem::size_of::<conduit_kernel::KernelEvent>() as u32,
            16,
            conduit_kernel::remote_sign_storage_bytes(16).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn projected_tables_execute_through_existing_kernel_delivery_acknowledgement() {
    let plan = fixtures::transfer_plan();
    let prepared = PreparedExecutionRegions::prepare(
        &plan,
        &plan.fragments[0].fragment_id,
        ExecutionRegionLimits {
            maximum_regions: 2,
            maximum_lanes: 2,
            maximum_nodes: 2,
            maximum_cords: 1,
        },
    )
    .unwrap();
    let source_tables = prepared.prepare_kernel_tables(0).unwrap();
    let sink_tables = prepared.prepare_kernel_tables(1).unwrap();
    assert_eq!(source_tables.original_nodes(), &[NodeId(0)]);
    assert_eq!(sink_tables.original_nodes(), &[NodeId(1)]);
    assert_eq!(source_tables.original_cords(), sink_tables.original_cords());
    assert_eq!(sink_tables.nodes()[0].input_cords[0], Some(CordId(0)));
    let endpoint = RemoteEndpointId(prepared.transfers()[0].cord().0);
    assert_eq!(
        source_tables.cords()[0].sink,
        CordEndpoint::Remote(endpoint)
    );
    assert_eq!(
        sink_tables.cords()[0].source,
        CordEndpoint::Remote(endpoint)
    );
    assert_eq!(
        sink_tables.cords()[0].sink,
        CordEndpoint::local(NodeId(0), PortId(0))
    );
    let mut source = install(&source_tables, true);
    let mut sink = install(&sink_tables, false);
    source.step().unwrap();
    let offer = source
        .remote_egress_offer(endpoint, CordId(0))
        .unwrap()
        .unwrap();
    source
        .remote_egress_accept(endpoint, CordId(0), offer.sequence)
        .unwrap();
    assert_eq!(source.cord_usage(CordId(0)).unwrap(), (1, 1));
    sink.admit_remote_input(
        endpoint,
        CordId(0),
        offer.sequence,
        source.values().get(offer.value).unwrap(),
    )
    .unwrap();
    assert_eq!(sink.cord_usage(CordId(0)).unwrap(), (1, 1));
    // Acceptance alone did not release the original source value.
    assert_eq!(source.values().used_items(), 1);
    source
        .remote_egress_delivered(endpoint, CordId(0), offer.sequence)
        .unwrap();
    assert_eq!(source.values().used_items(), 0);
    sink.step().unwrap();
    assert_eq!(sink.drivers()[0].received, 11);
    assert_eq!(sink.values().used_items(), 0);
}
