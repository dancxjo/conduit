use super::*;
use conduit_kernel::{
    scheduler::*, CordId, FixedRoutes, FixedSignLog, KernelEvent, NodeId, PortId, RouteRange,
    RouteTarget, ValueRef,
};
mod probe {
    use super::std;
    use std::thread_local;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../plot/tests/common/allocation_probe.rs"
    ));
}
#[global_allocator]
static ALLOCATOR: probe::Allocator = probe::Allocator;
struct Back<'a> {
    schema: &'a [u8],
    source: Option<ValueRef>,
    second: Option<ValueRef>,
    emitted: u8,
    delay: u8,
    received: u8,
    staged: bool,
    done: bool,
    seen: bool,
}
impl StepBack<1> for Back<'_> {
    fn canonical_input_schema(&self, _: PortId) -> Option<&[u8]> {
        self.source.is_none().then_some(self.schema)
    }
    fn canonical_output_schema(&self, _: PortId) -> Option<&[u8]> {
        self.source.is_some().then_some(self.schema)
    }
    fn step_committed(&mut self) {
        if self.staged {
            if self.source.is_some() {
                self.emitted += 1;
                self.done = self.emitted == 1 + u8::from(self.second.is_some());
            } else {
                self.seen = true;
                self.received += 1;
            }
            self.staged = false;
        }
    }
    fn step(&mut self, io: &mut StepIo<1>, input: &StepInputBytes<'_, 1>) -> StepOutcome {
        if let Some(value) = self.source {
            if self.done {
                return StepOutcome::Complete;
            }
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            let value = if self.emitted == 0 {
                value
            } else {
                self.second.unwrap()
            };
            io.send(PortId(0), value).unwrap();
            self.staged = true;
            return StepOutcome::Progress;
        }
        if io.input(PortId(0)).is_some() {
            if self.delay != 0 {
                self.delay -= 1;
                io.exhaust_fuel();
                return StepOutcome::Yield;
            }
            let parts = input
                .canonical_parts(PortId(0))
                .expect("explicit separated input");
            assert_eq!(parts.schema, self.schema);
            assert!(input.input(PortId(0)).is_none());
            let validator =
                Validator::new(self.schema, parts.schema.len() + parts.node.len()).unwrap();
            validator.validate_node(parts.node).unwrap();
            io.consume(PortId(0)).unwrap();
            self.staged = true;
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) {
            io.consume_closed(PortId(0)).unwrap();
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
}
fn scheduler<'a>(
    store: SchemaValueStore<'a, FixedValueStore<2, 16>>,
    value: ValueRef,
    source: &'a [u8],
    sink: &'a [u8],
    second: Option<ValueRef>,
    delay: u8,
) -> FixedScheduler<
    Back<'a>,
    SchemaValueStore<'a, FixedValueStore<2, 16>>,
    FixedSignLog<64>,
    2,
    1,
    1,
    1,
    2,
    1,
> {
    let mut routes = FixedRoutes::new(1);
    routes
        .install(
            NodeId(0),
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(0),
                sink: conduit_kernel::CordEndpoint::local(NodeId(1), PortId(0)),
            }],
        )
        .unwrap();
    routes.seal().unwrap();
    FixedScheduler::new(
        [
            NodeSpec {
                input_cords: [None],
                maximum_step_fuel: 8,
            },
            NodeSpec {
                input_cords: [Some(CordId(0))],
                maximum_step_fuel: 8,
            },
        ],
        [CordSpec::local(
            CordId(0),
            (NodeId(0), PortId(0)),
            (NodeId(1), PortId(0)),
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: value.byte_len,
                pressure_policy: Default::default(),
            },
        )],
        routes,
        [
            Back {
                schema: source,
                source: Some(value),
                second,
                emitted: 0,
                delay: 0,
                received: 0,
                staged: false,
                done: false,
                seen: false,
            },
            Back {
                schema: sink,
                source: None,
                second: None,
                emitted: 0,
                delay,
                received: 0,
                staged: false,
                done: false,
                seen: false,
            },
        ],
        store,
        FixedSignLog::new((64 * core::mem::size_of::<KernelEvent>()) as u32).unwrap(),
    )
    .unwrap()
}
#[test]
fn actual_scheduler_routes_validated_nodes_without_allocating_or_changing_logical_lengths() {
    let (schema, bytes) = fixture("test/local-schema-route", 9);
    let validator = Validator::new(&schema, bytes.len()).unwrap();
    let mut store =
        SchemaValueStore::try_new(canonical(), &[(validator, 1)], bytes.len() as u32, 4096)
            .unwrap();
    let value = store
        .store_structured(validator.validate(&bytes).unwrap())
        .unwrap();
    let mut kernel = scheduler(store, value, &schema, &schema, None, 0);
    let (result, cost) = probe::observe(|| kernel.run(32));
    result.unwrap();
    assert!(kernel.drivers()[1].seen);
    assert_eq!(cost.allocation_attempts, 0);
    assert_eq!(cost.reallocations, 0);
}
#[test]
fn mismatched_receiver_refuses_before_commit_or_value_retirement() {
    let (schema, bytes) = fixture("test/route-origin", 9);
    let (foreign, _) = fixture("test/route-foreign", 9);
    let validator = Validator::new(&schema, bytes.len()).unwrap();
    let mut store =
        SchemaValueStore::try_new(canonical(), &[(validator, 1)], bytes.len() as u32, 4096)
            .unwrap();
    let value = store
        .store_structured(validator.validate(&bytes).unwrap())
        .unwrap();
    let mut kernel = scheduler(store, value, &schema, &foreign, None, 0);
    assert_eq!(kernel.run(32), Err(SchedulerError::InvalidPlan));
    assert!(!kernel.drivers()[0].done);
    assert!(!kernel.drivers()[1].seen);
    assert_eq!(kernel.values().reference_count(value), Ok(1));
    let mut out = vec![0; bytes.len()];
    assert_eq!(
        kernel
            .values()
            .copy_canonical_into(value, &mut out)
            .unwrap(),
        bytes
    );
}

#[test]
fn queue_pressure_preserves_both_separated_values_until_the_consumer_can_commit() {
    let (schema, bytes) = fixture("test/pressured-schema-route", 9);
    let validator = Validator::new(&schema, bytes.len()).unwrap();
    let mut store =
        SchemaValueStore::try_new(canonical(), &[(validator, 2)], bytes.len() as u32 * 2, 4096)
            .unwrap();
    let first = store
        .store_structured(validator.validate(&bytes).unwrap())
        .unwrap();
    let second = store
        .store_structured(validator.validate(&bytes).unwrap())
        .unwrap();
    let mut kernel = scheduler(store, first, &schema, &schema, Some(second), 4);
    let (result, cost) = probe::observe(|| kernel.run(64));
    result.unwrap();
    assert_eq!(kernel.drivers()[0].emitted, 2);
    assert_eq!(kernel.drivers()[1].received, 2);
    assert_eq!(kernel.values().used_items(), 0);
    assert_eq!(cost.allocation_attempts, 0);
}

#[test]
fn separated_nodes_cannot_escape_through_a_remote_canonical_egress() {
    use conduit_kernel::{CordEndpoint, RemoteEndpointId};
    let (schema, bytes) = fixture("test/local-only-schema", 4);
    let validator = Validator::new(&schema, bytes.len()).unwrap();
    let mut store =
        SchemaValueStore::try_new(canonical(), &[(validator, 1)], bytes.len() as u32, 4096)
            .unwrap();
    let value = store
        .store_structured(validator.validate(&bytes).unwrap())
        .unwrap();
    let endpoint = RemoteEndpointId(0);
    let mut routes = FixedRoutes::<1, 1>::new(1);
    routes
        .install(
            NodeId(0),
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(0),
                sink: CordEndpoint::Remote(endpoint),
            }],
        )
        .unwrap();
    routes.seal().unwrap();
    let signs = FixedSignLog::<64>::new_with_remote_storage(
        (64 * core::mem::size_of::<KernelEvent>()) as u32,
        64,
        conduit_kernel::remote_sign_storage_bytes(64).unwrap(),
    )
    .unwrap();
    let mut kernel = FixedScheduler::<_, _, _, 1, 1, 1, 1, 1, 1>::new(
        [NodeSpec {
            input_cords: [None],
            maximum_step_fuel: 8,
        }],
        [CordSpec::remote_egress(
            CordId(0),
            (NodeId(0), PortId(0)),
            endpoint,
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: value.byte_len,
                pressure_policy: Default::default(),
            },
        )],
        routes,
        [Back {
            schema: &schema,
            source: Some(value),
            second: None,
            emitted: 0,
            delay: 0,
            received: 0,
            staged: false,
            done: false,
            seen: false,
        }],
        store,
        signs,
    )
    .unwrap();
    assert_eq!(kernel.run(16), Err(SchedulerError::InvalidPlan));
    assert!(!kernel.drivers()[0].done);
    assert_eq!(kernel.values().reference_count(value), Ok(1));
}
