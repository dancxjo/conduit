//! Transaction fault injection below the unchanged specialized topology binders.
//! These tests do not claim those binders admit mixed-output producers.
#![cfg(feature = "alloc")]
use super::super::{compact_f32::CompactF32Bindings, CompactF32Layout};
use super::*;
use crate::HostedValueStore;
use alloc::{boxed::Box, vec, vec::Vec};

#[derive(Clone, Copy)]
enum Representation {
    Native,
    Compact,
    Owned,
}
struct Back {
    bytes: Vec<u8>,
    malformed: bool,
    deny_owned: bool,
    specialized: PortId,
}
impl StepBack<PORTS> for Back {
    fn step(&mut self, _: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        StepOutcome::Await
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        if port == self.specialized {
            Some(&self.bytes)
        } else if self.malformed {
            Some(b"bad")
        } else {
            Some(b"data")
        }
    }
    fn prepared_compact_f32_output_mut(&mut self, port: PortId) -> Option<&mut Vec<u8>> {
        (port == self.specialized && !self.deny_owned).then_some(&mut self.bytes)
    }
}
type Play = FixedScheduler<Back, HostedValueStore, FixedSignLog<32>, 2, 2, PORTS, 2, 2, 2>;
fn prepare(
    kind: Representation,
    specialized: PortId,
    malformed: bool,
    deny_owned: bool,
) -> (Play, StepIo<PORTS>) {
    let shape = CompactF32Layout {
        lanes: 1,
        logical_bytes: 8,
    };
    let mut values = HostedValueStore::new(4, 8, 32).unwrap();
    values.prepare_native_u64();
    values.prepare_compact_f32();
    let owner = if matches!(kind, Representation::Owned) {
        Some(
            values
                .prepare_owned_compact_f32_outputs(&[(shape, 1)], &[8, 8, 8])
                .unwrap()
                .remove(0),
        )
    } else {
        None
    };
    let mut routes = FixedRoutes::new(PORTS as u16);
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
    let mut play = FixedScheduler::new(
        [
            node([None; PORTS]),
            node([Some(CordId(0)), Some(CordId(1))]),
        ],
        core::array::from_fn(|index| {
            CordSpec::local(
                CordId(index as u16),
                (NodeId(0), PortId(index as u16)),
                (NodeId(1), PortId(index as u16)),
                CordCapacity {
                    slot_start: index as u16,
                    item_capacity: 1,
                    byte_capacity: 8,
                    pressure_policy: Default::default(),
                },
            )
        }),
        routes,
        [
            Back {
                bytes: vec![0, 0, 128, 63],
                malformed,
                deny_owned,
                specialized,
            },
            Back {
                bytes: vec![],
                malformed: false,
                deny_owned: false,
                specialized,
            },
        ],
        values,
        FixedSignLog::new((32 * core::mem::size_of::<crate::KernelEvent>()) as u32).unwrap(),
    )
    .unwrap();
    // Deliberately construct the exact selected representation at the lower
    // materialization boundary. Public chain/DAG topology remains unchanged.
    let mut io = StepIo::test_frame([None; PORTS], [false; PORTS], [Some(8); PORTS], None, 4);
    match kind {
        Representation::Native => {
            play.native_output_ports[0] = Some(specialized);
            io.native_output_port = Some(specialized);
            io.send_native_u64(specialized, 42).unwrap();
        }
        Representation::Compact | Representation::Owned => {
            play.compact_f32_bindings = Some(Box::new(CompactF32Bindings {
                inputs: [[false; PORTS]; 2],
                outputs: [Some((specialized, shape)), None],
                owned: owner.map(|owner| Box::new([Some(owner), None])),
            }));
            io.compact_f32_output = Some((specialized, shape));
            io.send_compact_f32(specialized).unwrap();
        }
    }
    io.send_prepared(PortId(1 - specialized.0), 4).unwrap();
    (play, io)
}
#[test]
fn later_byte_materialization_failure_releases_native_and_compact_and_restores_owned_buffer() {
    for kind in [
        Representation::Native,
        Representation::Compact,
        Representation::Owned,
    ] {
        let (mut play, mut io) = prepare(kind, PortId(0), true, false);
        let original = play.drivers[0].bytes.as_ptr();
        assert!(matches!(
            play.materialize_step_output(0, &mut io),
            Err(SchedulerError::InvalidPlan)
        ));
        assert_eq!(play.values.used_items(), 0);
        assert_eq!(play.values.used_bytes(), 0);
        assert_eq!(play.drivers[0].bytes.as_ptr(), original);
        assert_eq!(play.drivers[0].bytes, [0, 0, 128, 63]);
        for index in 0..2 {
            assert_eq!(play.cord_usage(CordId(index)).unwrap(), (0, 0));
        }
        assert!(!play
            .signs
            .events()
            .any(|e| e.kind == KernelEventKind::ValueRouted));
        // The restored owner is still usable, not merely an equal byte copy.
        play.drivers[0].malformed = false;
        let (_, mut retry) = prepare(kind, PortId(0), false, false);
        let generated = play.materialize_step_output(0, &mut retry).unwrap();
        play.rollback_generated_output(0, generated, &retry)
            .unwrap();
        assert_eq!(play.values.used_items(), 0);
        assert_eq!(play.drivers[0].bytes.as_ptr(), original);
    }
}
#[test]
fn later_owned_buffer_refusal_releases_preceding_bytes_without_copy_fallback() {
    let (mut play, mut io) = prepare(Representation::Owned, PortId(1), false, true);
    let original = play.drivers[0].bytes.as_ptr();
    assert!(matches!(
        play.materialize_step_output(0, &mut io),
        Err(SchedulerError::InvalidPlan)
    ));
    assert_eq!(play.values.used_items(), 0);
    assert_eq!(play.values.used_bytes(), 0);
    assert_eq!(play.drivers[0].bytes.as_ptr(), original);
}
#[test]
fn post_materialization_refusal_rolls_back_both_orders_and_all_representations() {
    for kind in [
        Representation::Native,
        Representation::Compact,
        Representation::Owned,
    ] {
        for port in [PortId(0), PortId(1)] {
            let (mut play, io) = prepare(kind, port, false, false);
            let original = play.drivers[0].bytes.as_ptr();
            play.signs =
                FixedSignLog::new(core::mem::size_of::<crate::KernelEvent>() as u32).unwrap();
            assert!(matches!(
                play.apply_step(0, StepOutcome::Progress, io),
                Err(SchedulerError::Sign(_))
            ));
            assert_eq!(play.values.used_items(), 0);
            assert_eq!(play.values.used_bytes(), 0);
            assert_eq!(play.drivers[0].bytes.as_ptr(), original);
            assert_eq!(play.drivers[0].bytes, [0, 0, 128, 63]);
        }
    }
}
