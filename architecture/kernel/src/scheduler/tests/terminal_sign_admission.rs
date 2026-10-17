//! Terminal lifecycle evidence must fit before the Step commits.
use super::*;
use crate::SignError;
struct Terminal {
    abnormal: bool,
    committed: u8,
}
impl StepBack<PORTS> for Terminal {
    fn step(&mut self, _: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.abnormal {
            StepOutcome::Abnormal {
                port: PortId(0),
                terminal: CanonicalValue::new(b"lost").unwrap(),
            }
        } else {
            StepOutcome::Complete
        }
    }
    fn step_committed(&mut self) {
        self.committed += 1;
    }
}
type Scheduler =
    FixedScheduler<Terminal, FixedValueStore<2, 8>, FixedSignLog<16>, 1, 2, PORTS, 2, 2, 2>;
fn signs(remote: u16, ordinary: u32) -> FixedSignLog<16> {
    FixedSignLog::new_with_remote_storage(
        ordinary * core::mem::size_of::<crate::KernelEvent>() as u32,
        remote,
        crate::remote_sign_storage_bytes(remote).unwrap(),
    )
    .unwrap()
}
fn scheduler(abnormal: bool, remote: u16, ordinary: u32) -> Scheduler {
    let mut routes = FixedRoutes::new(PORTS as u16);
    let targets = core::array::from_fn::<_, 2, _>(|i| RouteTarget {
        cord: CordId(i as u16),
        sink: crate::CordEndpoint::Remote(RemoteEndpointId(i as u16)),
    });
    routes
        .install(
            NodeId(0),
            PortId(0),
            RouteRange { start: 0, len: 2 },
            &targets,
        )
        .unwrap();
    routes.seal().unwrap();
    Scheduler::new(
        [node([None; PORTS])],
        core::array::from_fn(|i| {
            CordSpec::remote_egress(
                CordId(i as u16),
                (NodeId(0), PortId(0)),
                RemoteEndpointId(i as u16),
                CordCapacity {
                    slot_start: i as u16,
                    item_capacity: 1,
                    byte_capacity: 8,
                    pressure_policy: Default::default(),
                },
            )
            .with_track(if abnormal {
                AssignedConnectionTrack::AbnormalTerminal
            } else {
                AssignedConnectionTrack::NormalClose
            })
        }),
        routes,
        [Terminal {
            abnormal,
            committed: 0,
        }],
        FixedValueStore::new(16).unwrap(),
        signs(remote, ordinary),
    )
    .unwrap()
}
fn refusal_and_retry(remote_pressure: bool) {
    for abnormal in [false, true] {
        let mut scheduler = if remote_pressure {
            scheduler(abnormal, 1, 16)
        } else {
            scheduler(abnormal, 2, 16)
        };
        if !remote_pressure {
            scheduler.signs.next_sequence = u32::MAX - 4;
        }
        assert_eq!(
            scheduler.step(),
            Err(SchedulerError::Sign(if remote_pressure {
                SignError::RemoteItemCapacityExceeded
            } else {
                SignError::SequenceOverflow
            }))
        );
        assert_eq!(scheduler.drivers[0].committed, 0);
        assert!(!scheduler.completed[0]);
        assert!(scheduler.ready.contains(0));
        assert_eq!(scheduler.values.used_items(), 0);
        assert_eq!(scheduler.signs.events().count(), 1);
        assert_eq!(
            scheduler.signs.events().next().unwrap().kind,
            KernelEventKind::StepFuelGranted
        );
        for cord in &scheduler.cords {
            assert_eq!(cord.len, 0);
            assert!(!cord.producer_closed);
        }
        scheduler.signs = signs(2, 16);
        assert_eq!(
            scheduler.step(),
            Ok(SchedulerStatus::Progress { node: NodeId(0) })
        );
        assert_eq!(scheduler.drivers[0].committed, 1);
        assert!(scheduler.completed[0]);
        assert_eq!(scheduler.signs.events().count(), 6);
        for cord in &scheduler.cords {
            assert_eq!(cord.len, 1);
            assert!(cord.producer_closed);
        }
    }
}
#[test]
fn remote_terminal_sign_pressure_refuses_before_step_commit() {
    refusal_and_retry(true);
}
#[test]
fn terminal_sign_sequence_reserves_both_value_and_remote_closure() {
    refusal_and_retry(false);
}
