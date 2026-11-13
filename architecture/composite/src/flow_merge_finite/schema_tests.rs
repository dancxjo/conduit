//! CPU conformance of the actual Merge Back in the schema-storage scheduler.
use super::*;
use conduit_core::{kind_id, StructuredFieldType, StructuredFieldValue, StructuredInfoValue};
use conduit_kernel::{
    scheduler::*, CordId, FixedRoutes, FixedSignLog, FixedValueStore, KernelEvent, NodeId,
    RouteRange, RouteTarget, ValueRef, ValueStorage,
};
use conduit_plan_lowering::schema_value_store::SchemaValueStore;

fn fixture() -> (StructuredInfoType, &'static [u8], [Vec<u8>; 2]) {
    let number = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
    let boolean = StructuredInfoType::leaf(kind_id("value/bool")).unwrap();
    let ty = StructuredInfoType::record(
        kind_id("test/merge-schema-custody"),
        vec![
            StructuredFieldType::new("number", number.clone()).unwrap(),
            StructuredFieldType::new("unselected", boolean.clone()).unwrap(),
        ],
    )
    .unwrap();
    let bytes = [1_u64, 2].map(|n| {
        StructuredInfoValue::record(
            ty.clone(),
            vec![
                StructuredFieldValue::new(
                    "number",
                    StructuredInfoValue::leaf(number.clone(), n.to_le_bytes().to_vec()).unwrap(),
                )
                .unwrap(),
                StructuredFieldValue::new(
                    "unselected",
                    StructuredInfoValue::leaf(boolean.clone(), vec![1]).unwrap(),
                )
                .unwrap(),
            ],
        )
        .unwrap()
        .canonical_bytes()
        .unwrap()
    });
    let linked = Box::leak(ty.canonical_bytes().unwrap().into_boxed_slice());
    (ty, linked, bytes)
}

enum Back<'a> {
    Source {
        schema: &'a [u8],
        bytes: &'a [u8],
        separated: bool,
        count: u8,
        limit: u8,
        staged: bool,
    },
    Merge {
        back: FlowMergeFiniteBack,
        canonical: u8,
        separated: u8,
        blocked: u16,
    },
    Sink {
        schema: &'a [u8],
        expected: [&'a [u8]; 2],
        accepts_schema: bool,
        expects_parts: bool,
        delay: u8,
        seen: [u8; 4],
        count: usize,
        staged: Option<u8>,
    },
}
impl StepBack<2> for Back<'_> {
    fn canonical_input_schema(&self, port: PortId) -> Option<&[u8]> {
        match self {
            Self::Merge { back, .. } => {
                <FlowMergeFiniteBack as StepBack<2>>::canonical_input_schema(back, port)
            }
            Self::Sink {
                schema,
                accepts_schema: true,
                ..
            } if port == PortId(0) => Some(schema),
            _ => None,
        }
    }
    fn canonical_output_schema(&self, port: PortId) -> Option<&[u8]> {
        match self {
            Self::Merge { back, .. } => {
                <FlowMergeFiniteBack as StepBack<2>>::canonical_output_schema(back, port)
            }
            Self::Source {
                schema,
                separated: true,
                ..
            } if port == PortId(0) => Some(schema),
            _ => None,
        }
    }
    fn terminal_transductions(&self) -> [Option<AssignedTerminalTransduction>; 2] {
        match self {
            Self::Merge { back, .. } => back.terminal_transductions(),
            _ => [None; 2],
        }
    }
    fn step(&mut self, io: &mut StepIo<2>, input: &StepInputBytes<'_, 2>) -> StepOutcome {
        match self {
            Self::Source {
                bytes,
                count,
                limit,
                staged,
                ..
            } => {
                if count == limit {
                    return StepOutcome::Complete;
                }
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send_prepared(PortId(0), bytes.len() as u32).unwrap();
                *staged = true;
                StepOutcome::Progress
            }
            Self::Merge {
                back,
                canonical,
                separated,
                blocked,
            } => {
                let ready = io.output_ready(PortId(0));
                let outcome = back.step(io, input);
                for port in [PortId(0), PortId(1)] {
                    if !ready {
                        assert!(!io.test_consumed(port));
                    }
                    if io.test_consumed(port) {
                        *canonical += u8::from(input.input(port).is_some());
                        *separated += u8::from(input.canonical_parts(port).is_some());
                    }
                }
                if !ready && (io.input(PortId(0)).is_some() || io.input(PortId(1)).is_some()) {
                    *blocked += 1;
                }
                outcome
            }
            Self::Sink {
                schema,
                expected,
                expects_parts,
                delay,
                staged,
                ..
            } => {
                if io.input(PortId(0)).is_none() {
                    if io.input_closed(PortId(0)) {
                        io.consume_closed(PortId(0)).unwrap();
                        return StepOutcome::Complete;
                    }
                    return StepOutcome::Await;
                }
                if *delay != 0 {
                    *delay -= 1;
                    io.exhaust_fuel();
                    return StepOutcome::Yield;
                }
                let marker = if *expects_parts {
                    let parts = input
                        .canonical_parts(PortId(0))
                        .expect("separated merged output");
                    assert!(input.input(PortId(0)).is_none());
                    assert_eq!(parts.schema, *schema);
                    if parts.node == &expected[0][schema.len()..] {
                        1
                    } else {
                        assert_eq!(parts.node, &expected[1][schema.len()..]);
                        2
                    }
                } else {
                    assert!(input.canonical_parts(PortId(0)).is_none());
                    let bytes = input.input(PortId(0)).expect("canonical fallback output");
                    if bytes == expected[0] {
                        1
                    } else {
                        assert_eq!(bytes, expected[1]);
                        2
                    }
                };
                assert_eq!(
                    io.input(PortId(0)).unwrap().byte_len as usize,
                    expected[0].len()
                );
                io.consume(PortId(0)).unwrap();
                *staged = Some(marker);
                StepOutcome::Progress
            }
        }
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        match self {
            Self::Source {
                bytes,
                staged: true,
                ..
            } if port == PortId(0) => Some(bytes),
            Self::Merge { back, .. } => {
                <FlowMergeFiniteBack as StepBack<2>>::prepared_output(back, port)
            }
            _ => None,
        }
    }
    fn step_committed(&mut self) {
        match self {
            Self::Source { count, staged, .. } => {
                *count += u8::from(*staged);
                *staged = false;
            }
            Self::Merge { back, .. } => <FlowMergeFiniteBack as StepBack<2>>::step_committed(back),
            Self::Sink {
                seen,
                count,
                staged,
                ..
            } => {
                if let Some(marker) = staged.take() {
                    seen[*count] = marker;
                    *count += 1;
                }
            }
        }
    }
}

#[test]
fn scheduler_merges_mixed_representations_and_falls_back_for_canonical_fanout() {
    for (fallback, early_left_close) in [(false, false), (true, false), (false, true)] {
        let (ty, schema, bytes) = fixture();
        let maximum = bytes[0].len() as u32;
        let validator = BorrowedStructuredValueValidator::new(schema, maximum as usize).unwrap();
        let store = SchemaValueStore::try_new(
            FixedValueStore::<8, 1024>::new(8192).unwrap(),
            &[(validator, 8)],
            maximum * 8,
            16384,
        )
        .unwrap();
        let mut routes = FixedRoutes::<5, 4>::new(2);
        for (source, targets, start) in [
            (0, vec![(0, 2, 0)], 0),
            (1, vec![(1, 2, 1)], 1),
            (2, vec![(2, 3, 0), (3, 4, 0)], 2),
        ] {
            let targets: Vec<_> = targets
                .into_iter()
                .map(|(cord, node, port)| RouteTarget {
                    cord: CordId(cord),
                    sink: conduit_kernel::CordEndpoint::local(NodeId(node), PortId(port)),
                })
                .collect();
            routes
                .install(
                    NodeId(source),
                    PortId(0),
                    RouteRange {
                        start,
                        len: targets.len() as u16,
                    },
                    &targets,
                )
                .unwrap();
        }
        routes.seal().unwrap();
        let sink = |accepts_schema, delay| Back::Sink {
            schema,
            expected: [&bytes[0], &bytes[1]],
            accepts_schema,
            expects_parts: !fallback,
            delay,
            seen: [0; 4],
            count: 0,
            staged: None,
        };
        let drivers = [
            Back::Source {
                schema,
                bytes: &bytes[0],
                separated: true,
                count: 0,
                limit: if early_left_close { 1 } else { 2 },
                staged: false,
            },
            Back::Source {
                schema,
                bytes: &bytes[1],
                separated: false,
                count: 0,
                limit: if early_left_close { 3 } else { 2 },
                staged: false,
            },
            Back::Merge {
                back: FlowMergeFiniteBack::prepare_static(&ty, schema, maximum).unwrap(),
                canonical: 0,
                separated: 0,
                blocked: 0,
            },
            sink(true, 12),
            sink(!fallback, 0),
        ];
        let endpoints = [(0, 2, 0), (1, 2, 1), (2, 3, 0), (2, 4, 0)];
        let mut kernel = FixedScheduler::<_, _, _, 5, 4, 2, 4, 5, 4, 5, 1>::new(
            [
                NodeSpec {
                    input_cords: [None; 2],
                    maximum_step_fuel: 8,
                },
                NodeSpec {
                    input_cords: [None; 2],
                    maximum_step_fuel: 8,
                },
                NodeSpec {
                    input_cords: [Some(CordId(0)), Some(CordId(1))],
                    maximum_step_fuel: 8,
                },
                NodeSpec {
                    input_cords: [Some(CordId(2)), None],
                    maximum_step_fuel: 8,
                },
                NodeSpec {
                    input_cords: [Some(CordId(3)), None],
                    maximum_step_fuel: 8,
                },
            ],
            core::array::from_fn(|n| {
                let (source, sink, port) = endpoints[n];
                CordSpec::local(
                    CordId(n as u16),
                    (NodeId(source), PortId(0)),
                    (NodeId(sink), PortId(port)),
                    CordCapacity {
                        slot_start: n as u16,
                        item_capacity: 1,
                        byte_capacity: maximum,
                        pressure_policy: Default::default(),
                    },
                )
            }),
            routes,
            drivers,
            store,
            FixedSignLog::<512>::new((512 * core::mem::size_of::<KernelEvent>()) as u32).unwrap(),
        )
        .unwrap();
        let allocations =
            crate::test_support::allocation::allocations_during(|| kernel.run(1024).unwrap());
        assert_eq!(allocations, 0);
        let Back::Merge {
            canonical,
            separated,
            blocked,
            ..
        } = &kernel.drivers()[2]
        else {
            unreachable!()
        };
        assert_eq!(
            (*canonical, *separated),
            if early_left_close { (3, 1) } else { (2, 2) }
        );
        assert!(*blocked > 0, "slow sink must exert real output pressure");
        for sink in &kernel.drivers()[3..] {
            let Back::Sink { seen, count, .. } = sink else {
                unreachable!()
            };
            assert_eq!(*count, 4);
            assert_eq!(
                seen,
                if early_left_close {
                    &[1, 2, 2, 2]
                } else {
                    &[1, 2, 1, 2]
                }
            );
        }
        assert_eq!(kernel.values().used_items(), 0);
        assert_eq!(kernel.values().used_bytes(), 0);
    }
}

#[test]
fn linked_merge_rejects_bad_unselected_field_or_schema_before_consumption() {
    let (ty, schema, bytes) = fixture();
    let mut malformed = bytes[0].clone();
    *malformed.last_mut().unwrap() = 2; // Bool field not used for routing.
    let mut foreign = bytes[0].clone();
    foreign[0] ^= 1;
    for bytes in [&malformed, &foreign] {
        let mut back = FlowMergeFiniteBack::prepare_static(&ty, schema, 1024).unwrap();
        let reference = ValueRef {
            slot: 0,
            generation: 1,
            byte_len: bytes.len() as u32,
        };
        let mut io = StepIo::test_frame(
            [Some(reference), None],
            [false; 2],
            [Some(1024), None],
            None,
            16,
        );
        let allocations = crate::test_support::allocation::allocations_during(|| {
            assert!(matches!(
                back.step(
                    &mut io,
                    &StepInputBytes::test_frame([Some(bytes), None], None)
                ),
                StepOutcome::Fail(_)
            ));
            assert!(!io.test_consumed(PortId(0)));
            assert!(
                <FlowMergeFiniteBack as StepBack<2>>::prepared_output(&back, PortId(0)).is_none()
            );
        });
        assert_eq!(allocations, 0);
        assert_eq!(back.preferred, 0);
        assert_eq!(back.committed_close, [false; 2]);
    }
}

#[test]
fn partial_byte_capacity_preserves_input_and_preference_until_whole_frame_fits() {
    let (ty, schema, bytes) = fixture();
    let mut back = FlowMergeFiniteBack::prepare_static(&ty, schema, 1024).unwrap();
    let reference = ValueRef {
        slot: 0,
        generation: 1,
        byte_len: bytes[0].len() as u32,
    };
    let input = StepInputBytes::test_frame([Some(bytes[0].as_slice()), None], None);
    let mut blocked = StepIo::test_frame(
        [Some(reference), None],
        [false; 2],
        [Some(reference.byte_len - 1), None],
        None,
        16,
    );
    assert_eq!(back.step(&mut blocked, &input), StepOutcome::Await);
    assert!(!blocked.test_consumed(PortId(0)));
    assert_eq!(back.preferred, 0);
    assert!(<FlowMergeFiniteBack as StepBack<2>>::prepared_output(&back, PortId(0)).is_none());
    let mut ready = StepIo::test_frame(
        [Some(reference), None],
        [false; 2],
        [Some(reference.byte_len), None],
        None,
        16,
    );
    assert_eq!(back.step(&mut ready, &input), StepOutcome::Progress);
    assert!(ready.test_consumed(PortId(0)));
    assert_eq!(
        <FlowMergeFiniteBack as StepBack<2>>::prepared_output(&back, PortId(0)),
        Some(bytes[0].as_slice())
    );
    <FlowMergeFiniteBack as StepBack<2>>::step_committed(&mut back);
    assert_eq!(back.preferred, 1);
}
