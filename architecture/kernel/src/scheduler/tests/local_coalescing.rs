//! Actual local Steps honor coalescing, including atomic mixed-policy fan-out.
use super::*;
#[derive(Clone, Copy)]
struct Back {
    source: bool,
    committed: usize,
}
impl StepBack<PORTS> for Back {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if !self.source || self.committed == 3 || !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let bytes: &[u8] = match self.committed {
            0 => b"old",
            1 => b"new",
            _ => b"last",
        };
        io.send_canonical(PortId(0), CanonicalValue::new(bytes).unwrap())
            .unwrap();
        StepOutcome::Progress
    }
    fn step_committed(&mut self) {
        if self.source {
            self.committed += 1;
        }
    }
}
#[test]
fn local_steps_coalesce_newest_values_and_preserve_blocked_fanout() {
    for blocking_branch in [false, true] {
        let mut routes = FixedRoutes::<3, 2>::new(PORTS as u16);
        routes
            .install(
                NodeId(0),
                PortId(0),
                RouteRange { start: 0, len: 2 },
                &core::array::from_fn::<_, 2, _>(|i| RouteTarget {
                    cord: CordId(i as u16),
                    sink: crate::CordEndpoint::local(NodeId((i + 1) as u16), PortId(0)),
                }),
            )
            .unwrap();
        routes.seal().unwrap();
        let mut scheduler = FixedScheduler::<_, _, _, 3, 2, PORTS, 4, 3, 2>::new(
            [
                node([None; PORTS]),
                node([Some(CordId(0)), None]),
                node([Some(CordId(1)), None]),
            ],
            core::array::from_fn(|i| {
                CordSpec::local(
                    CordId(i as u16),
                    (NodeId(0), PortId(0)),
                    (NodeId((i + 1) as u16), PortId(0)),
                    CordCapacity {
                        slot_start: (i * 2) as u16,
                        item_capacity: 2,
                        byte_capacity: 8,
                        pressure_policy: if blocking_branch && i == 1 {
                            Default::default()
                        } else {
                            AssignedPressurePolicy::CoalesceLatest
                        },
                    },
                )
            }),
            routes,
            core::array::from_fn(|i| Back {
                source: i == 0,
                committed: 0,
            }),
            FixedValueStore::<3, 8>::new(24).unwrap(),
            FixedSignLog::<64>::new(64 * core::mem::size_of::<crate::KernelEvent>() as u32)
                .unwrap(),
        )
        .unwrap();
        for _ in 0..12 {
            scheduler.step().unwrap();
        }
        assert_eq!(
            scheduler.drivers[0].committed,
            if blocking_branch { 2 } else { 3 }
        );
        assert_eq!(scheduler.values.used_items(), 2);
        for cord in 0..2 {
            assert_eq!(
                scheduler.cord_usage(CordId(cord as u16)).unwrap(),
                (2, if blocking_branch { 6 } else { 7 })
            );
            let first = scheduler.peek(cord).unwrap().unwrap();
            assert_eq!(scheduler.values.get(first), Ok(b"old".as_slice()));
            assert_eq!(scheduler.values.reference_count(first), Ok(2));
        }
        // Remove both oldest references, leaving the exact newest shared item.
        for cord in 0..2 {
            let first = scheduler.pop(cord).unwrap();
            scheduler.values.release(first).unwrap();
        }
        let newest = scheduler.peek(0).unwrap().unwrap();
        assert_eq!(scheduler.peek(1).unwrap(), Some(newest));
        assert_eq!(
            scheduler.values.get(newest),
            Ok(if blocking_branch {
                b"new".as_slice()
            } else {
                b"last".as_slice()
            })
        );
        assert_eq!(scheduler.values.reference_count(newest), Ok(2));
        assert_eq!(scheduler.values.used_items(), 1);
        assert_eq!(
            scheduler
                .signs
                .events()
                .filter(|e| e.kind == KernelEventKind::ValueRouted)
                .count(),
            if blocking_branch { 4 } else { 6 }
        );
        scheduler.cancel().unwrap();
        assert_eq!(scheduler.values.used_items(), 0);
    }
}
