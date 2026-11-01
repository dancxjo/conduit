//! Finite terminal transduction must count every distinct prepared output.
use super::*;
#[derive(Clone, Copy)]
struct Flush {
    source: bool,
    bound: AssignedFiniteTerminalEmission,
    committed: bool,
}
impl StepBack<PORTS> for Flush {
    fn terminal_transduction(&self) -> Option<AssignedTerminalTransduction> {
        self.source.then_some(AssignedTerminalTransduction {
            input: PortId(0),
            output: PortId(0),
            normal_close: AssignedNormalCloseTransduction::FlushThenPropagate(self.bound),
            abnormal: AssignedAbnormalTransduction::NotAccepted,
            cancellation: AssignedCancellationTransduction::NotCancellable,
        })
    }
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if !self.source || !io.input_closed(PortId(0)) {
            return StepOutcome::Await;
        }
        io.consume_closed(PortId(0)).unwrap();
        io.send_prepared(PortId(0), 4).unwrap();
        io.send_prepared(PortId(1), 4).unwrap();
        StepOutcome::Complete
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        match port.0 {
            0 => Some(b"data"),
            1 => Some(b"fact"),
            _ => None,
        }
    }
    fn step_committed(&mut self) {
        self.committed = true;
    }
}
#[test]
fn terminal_pair_respects_aggregate_item_and_byte_allowances_before_publication() {
    for (items, bytes, accepted) in [(2, 8, true), (1, 8, false), (2, 7, false)] {
        let endpoint = RemoteEndpointId(0);
        let mut routes = FixedRoutes::<2, 2>::new(PORTS as u16);
        for index in 0..2 {
            routes
                .install(
                    NodeId(0),
                    PortId(index),
                    RouteRange {
                        start: index,
                        len: 1,
                    },
                    &[RouteTarget {
                        cord: CordId(index),
                        sink: crate::CordEndpoint::local(NodeId(1), PortId(index)),
                    }],
                )
                .unwrap();
        }
        routes.seal().unwrap();
        let capacity = |slot_start| CordCapacity {
            slot_start,
            item_capacity: 1,
            byte_capacity: 4,
            pressure_policy: Default::default(),
        };
        let bound = AssignedFiniteTerminalEmission {
            maximum_items: items,
            maximum_bytes: bytes,
        };
        let mut source = node([Some(CordId(2)), None]);
        source.maximum_step_fuel = 3;
        let mut play = FixedScheduler::<_, _, _, 2, 3, PORTS, 3, 2, 2>::new(
            [source, node([Some(CordId(0)), Some(CordId(1))])],
            [
                CordSpec::local(
                    CordId(0),
                    (NodeId(0), PortId(0)),
                    (NodeId(1), PortId(0)),
                    capacity(0),
                ),
                CordSpec::local(
                    CordId(1),
                    (NodeId(0), PortId(1)),
                    (NodeId(1), PortId(1)),
                    capacity(1),
                ),
                CordSpec::remote_ingress(CordId(2), endpoint, (NodeId(0), PortId(0)), capacity(2)),
            ],
            routes,
            [
                Flush {
                    source: true,
                    bound,
                    committed: false,
                },
                Flush {
                    source: false,
                    bound,
                    committed: false,
                },
            ],
            FixedValueStore::<2, 8>::new(8).unwrap(),
            FixedSignLog::<32>::new_with_remote_storage(
                (32 * core::mem::size_of::<crate::KernelEvent>()) as u32,
                8,
                crate::remote_sign_storage_bytes(8).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let contracts = core::array::from_fn(|index| play.drivers[index].terminal_transductions());
        play.bind_terminal_transductions(contracts).unwrap();
        play.close_remote_input(endpoint, CordId(2)).unwrap();
        let result = play.step();
        if accepted {
            result.unwrap();
            assert!(play.drivers[0].committed);
            assert_eq!(play.values.used_items(), 2);
            for index in 0..2 {
                assert_eq!(play.cord_usage(CordId(index)).unwrap(), (1, 4));
            }
        } else {
            assert_eq!(result, Err(SchedulerError::InvalidPlan));
            assert!(!play.drivers[0].committed);
            assert_eq!(play.values.used_items(), 0);
            for index in 0..2 {
                assert_eq!(play.cord_usage(CordId(index)).unwrap(), (0, 0));
            }
            assert!(!play
                .signs
                .events()
                .any(|e| e.kind == KernelEventKind::ValueRouted));
        }
    }
}
