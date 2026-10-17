//! Semantic refusal must precede generated-output storage pressure.
use super::*;
#[derive(Clone, Copy)]
struct Generated {
    source: bool,
    prepared: bool,
    committed: u8,
}
impl StepBack<PORTS> for Generated {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if !self.source {
            return StepOutcome::Await;
        }
        if self.prepared {
            io.send_prepared(PortId(0), 5).unwrap();
        } else {
            io.send_canonical(PortId(0), CanonicalValue::new(b"fives").unwrap())
                .unwrap();
        }
        StepOutcome::Progress
    }
    fn prepared_output(&self, _: PortId) -> Option<&[u8]> {
        Some(b"fives")
    }
    fn step_committed(&mut self) {
        self.committed += 1;
    }
}
#[test]
fn generated_output_semantic_envelope_precedes_full_storage() {
    for prepared in [false, true] {
        for full in [false, true] {
            let mut values = FixedValueStore::<1, 8>::new(8).unwrap();
            let sentinel = full.then(|| values.store(b"held").unwrap());
            let mut routes = FixedRoutes::<2, 1>::new(PORTS as u16);
            routes
                .install(
                    NodeId(0),
                    PortId(0),
                    RouteRange { start: 0, len: 1 },
                    &[RouteTarget {
                        cord: CordId(0),
                        sink: crate::CordEndpoint::local(NodeId(1), PortId(0)),
                    }],
                )
                .unwrap();
            routes.seal().unwrap();
            let mut scheduler = FixedScheduler::<_, _, _, 2, 1, PORTS, 1, 2, 1>::new(
                [node([None; PORTS]), node([Some(CordId(0)), None])],
                [CordSpec::local(
                    CordId(0),
                    (NodeId(0), PortId(0)),
                    (NodeId(1), PortId(0)),
                    CordCapacity {
                        slot_start: 0,
                        item_capacity: 1,
                        byte_capacity: 8,
                        pressure_policy: Default::default(),
                    },
                )
                .with_maximum_value_bytes(4)],
                routes,
                core::array::from_fn(|i| Generated {
                    source: i == 0,
                    prepared,
                    committed: 0,
                }),
                values,
                FixedSignLog::<16>::new(16 * core::mem::size_of::<crate::KernelEvent>() as u32)
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(
                scheduler.step(),
                Err(SchedulerError::SemanticValueBoundExceeded)
            );
            assert_eq!(scheduler.drivers[0].committed, 0);
            assert!(scheduler.ready.contains(0));
            assert_eq!(scheduler.cord_usage(CordId(0)).unwrap(), (0, 0));
            assert_eq!(scheduler.values.used_items(), u16::from(full));
            assert_eq!(scheduler.signs.events().count(), 1);
            if let Some(value) = sentinel {
                assert_eq!(scheduler.values.get(value), Ok(b"held".as_slice()));
                assert_eq!(scheduler.values.reference_count(value), Ok(1));
            }
        }
    }
}
