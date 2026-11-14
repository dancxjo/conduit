//! Actual prepared-byte publication, rather than preloaded typed ValueRefs.
use super::*;
struct ProducerBack<'a> {
    schema: &'a [u8],
    bytes: &'a [u8],
    source: bool,
    accepts_schema: bool,
    expects_parts: bool,
    delay: u8,
    committed: u8,
    staged: bool,
}
impl StepBack<1> for ProducerBack<'_> {
    fn canonical_input_schema(&self, _: PortId) -> Option<&[u8]> {
        (!self.source && self.accepts_schema).then_some(self.schema)
    }
    fn canonical_output_schema(&self, _: PortId) -> Option<&[u8]> {
        self.source.then_some(self.schema)
    }
    fn step(&mut self, io: &mut StepIo<1>, input: &StepInputBytes<'_, 1>) -> StepOutcome {
        self.staged = false;
        if self.source {
            if self.committed == 2 {
                return StepOutcome::Complete;
            }
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            io.send_prepared(PortId(0), self.bytes.len() as u32)
                .unwrap();
            self.staged = true;
            return StepOutcome::Progress;
        }
        if io.input(PortId(0)).is_none() {
            if io.input_closed(PortId(0)) {
                io.consume_closed(PortId(0)).unwrap();
                return StepOutcome::Complete;
            }
            return StepOutcome::Await;
        }
        if self.delay > 0 {
            self.delay -= 1;
            io.exhaust_fuel();
            return StepOutcome::Yield;
        }
        if self.expects_parts {
            let parts = input.canonical_parts(PortId(0)).unwrap();
            assert_eq!(parts.schema, self.schema);
            assert_eq!(parts.node, &self.bytes[self.schema.len()..]);
            assert!(input.input(PortId(0)).is_none());
        } else {
            assert_eq!(input.input(PortId(0)), Some(self.bytes));
            assert!(input.canonical_parts(PortId(0)).is_none());
        }
        io.consume(PortId(0)).unwrap();
        self.staged = true;
        StepOutcome::Progress
    }
    fn prepared_output(&self, _: PortId) -> Option<&[u8]> {
        (self.source && self.staged).then_some(self.bytes)
    }
    fn step_committed(&mut self) {
        self.committed += u8::from(self.staged);
        self.staged = false;
    }
}
#[test]
fn prepared_outputs_use_node_storage_only_when_every_local_receiver_accepts_it() {
    for (mixed, invalid) in [(false, false), (true, false), (false, true)] {
        let (schema, mut bytes) = fixture("test/produced-canonical", 42);
        if invalid {
            bytes[0] ^= 1;
        }
        let validator = Validator::new(&schema, bytes.len()).unwrap();
        // The separated run has no canonical byte quota capable of holding a
        // frame. Its successful publication must use the original typed pool.
        let canonical = FixedValueStore::<4, 256>::new(if mixed { 1024 } else { 1 }).unwrap();
        let store =
            SchemaValueStore::try_new(canonical, &[(validator, 1)], bytes.len() as u32, 4096)
                .unwrap();
        let mut routes = FixedRoutes::<3, 2>::new(1);
        routes
            .install(
                NodeId(0),
                PortId(0),
                RouteRange { start: 0, len: 2 },
                &[
                    RouteTarget {
                        cord: CordId(0),
                        sink: conduit_kernel::CordEndpoint::local(NodeId(1), PortId(0)),
                    },
                    RouteTarget {
                        cord: CordId(1),
                        sink: conduit_kernel::CordEndpoint::local(NodeId(2), PortId(0)),
                    },
                ],
            )
            .unwrap();
        routes.seal().unwrap();
        let drivers = core::array::from_fn(|n| ProducerBack {
            schema: &schema,
            bytes: &bytes,
            source: n == 0,
            accepts_schema: n != 2 || !mixed,
            expects_parts: !mixed,
            delay: if n == 1 { 4 } else { 0 },
            committed: 0,
            staged: false,
        });
        let mut kernel = FixedScheduler::<_, _, _, 3, 2, 1, 2, 3, 2, 3, 1>::new(
            core::array::from_fn(|n| NodeSpec {
                input_cords: [if n == 0 {
                    None
                } else {
                    Some(CordId(n as u16 - 1))
                }],
                maximum_step_fuel: 8,
            }),
            core::array::from_fn(|n| {
                CordSpec::local(
                    CordId(n as u16),
                    (NodeId(0), PortId(0)),
                    (NodeId(n as u16 + 1), PortId(0)),
                    CordCapacity {
                        slot_start: n as u16,
                        item_capacity: 1,
                        byte_capacity: bytes.len() as u32,
                        pressure_policy: Default::default(),
                    },
                )
            }),
            routes,
            drivers,
            store,
            FixedSignLog::<128>::new((128 * core::mem::size_of::<KernelEvent>()) as u32).unwrap(),
        )
        .unwrap();
        let (result, cost) = probe::observe(|| kernel.run(128));
        if invalid {
            assert_eq!(result, Err(E::RepresentationMismatch.into()));
        } else {
            result.unwrap();
        }
        assert_eq!((cost.allocations, cost.reallocations), (0, 0));
        assert!(kernel
            .drivers()
            .iter()
            .all(|b| b.committed == if invalid { 0 } else { 2 }));
        assert_eq!(kernel.values().used_items(), 0);
        assert_eq!(kernel.values().used_bytes(), 0);
    }
}

#[test]
fn prepared_remote_egress_keeps_the_full_canonical_value() {
    use conduit_kernel::{CordEndpoint, RemoteEndpointId};
    let (schema, bytes) = fixture("test/prepared-remote-canonical", 9);
    let validator = Validator::new(&schema, bytes.len()).unwrap();
    let store = SchemaValueStore::try_new(
        FixedValueStore::<2, 256>::new(512).unwrap(),
        &[(validator, 1)],
        bytes.len() as u32,
        4096,
    )
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
                byte_capacity: bytes.len() as u32,
                pressure_policy: Default::default(),
            },
        )],
        routes,
        [ProducerBack {
            schema: &schema,
            bytes: &bytes,
            source: true,
            accepts_schema: false,
            expects_parts: false,
            delay: 0,
            committed: 0,
            staged: false,
        }],
        store,
        signs,
    )
    .unwrap();
    let (result, cost) = probe::observe(|| kernel.step());
    result.unwrap();
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    let offer = kernel
        .remote_egress_offer(endpoint, CordId(0))
        .unwrap()
        .unwrap();
    assert!(kernel
        .values()
        .canonical_parts(offer.value)
        .unwrap()
        .is_none());
    assert_eq!(kernel.host_value(offer.value).unwrap(), bytes);
    kernel
        .remote_egress_accept(endpoint, CordId(0), offer.sequence)
        .unwrap();
    kernel
        .remote_egress_delivered(endpoint, CordId(0), offer.sequence)
        .unwrap();
    assert_eq!(kernel.values().used_items(), 0);
}
