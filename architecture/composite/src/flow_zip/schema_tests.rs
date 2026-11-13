//! Actual Zip execution with separated inputs, canonical fallback and finite closure.
use super::*;
use conduit_core::{
    kind_id, BorrowedStructuredValueValidator, StructuredFieldType, StructuredFieldValue,
    StructuredInfoValue,
};
use conduit_kernel::{
    scheduler::*, CordId, FixedRoutes, FixedSignLog, FixedValueStore, KernelEvent, NodeId,
    RouteRange, RouteTarget, ValueStorage,
};
use conduit_plan_lowering::schema_value_store::SchemaValueStore;
fn fixture() -> (StructuredInfoType, &'static [u8], [Vec<u8>; 2]) {
    let number = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
    let boolean = StructuredInfoType::leaf(kind_id("value/bool")).unwrap();
    let ty = StructuredInfoType::record(
        kind_id("test/zip-schema-custody"),
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
        close_delay: u8,
        staged: bool,
    },
    Zip {
        back: FlowZipBack,
        separated: u8,
        canonical: u8,
    },
    Sink {
        schema: &'a [u8],
        expected: &'a [u8],
        separated: bool,
        delay: u8,
        count: u8,
        staged: bool,
    },
}
impl StepBack<2> for Back<'_> {
    fn canonical_input_schema(&self, port: PortId) -> Option<&[u8]> {
        match self {
            Self::Zip { back, .. } => {
                <FlowZipBack as StepBack<2>>::canonical_input_schema(back, port)
            }
            Self::Sink {
                schema,
                separated: true,
                ..
            } if port == PortId(0) => Some(schema),
            _ => None,
        }
    }
    fn canonical_output_schema(&self, port: PortId) -> Option<&[u8]> {
        match self {
            Self::Zip { back, .. } => {
                <FlowZipBack as StepBack<2>>::canonical_output_schema(back, port)
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
            Self::Zip { back, .. } => back.terminal_transductions(),
            _ => [None; 2],
        }
    }
    fn step(&mut self, io: &mut StepIo<2>, input: &StepInputBytes<'_, 2>) -> StepOutcome {
        match self {
            Self::Source {
                bytes,
                count,
                close_delay,
                staged,
                ..
            } => {
                if *count == 2 {
                    // Keep the finite producers open until both pairs can form;
                    // shortest-stream closure may otherwise discard an unmatched member.
                    if *close_delay > 0 {
                        *close_delay -= 1;
                        io.exhaust_fuel();
                        return StepOutcome::Yield;
                    }
                    return StepOutcome::Complete;
                }
                if !io.output_ready_for(PortId(0), bytes.len() as u32) {
                    return StepOutcome::Await;
                }
                io.send_prepared(PortId(0), bytes.len() as u32).unwrap();
                *staged = true;
                StepOutcome::Progress
            }
            Self::Zip {
                back,
                separated,
                canonical,
            } => {
                let result = back.step(io, input);
                for port in [PortId(0), PortId(1)] {
                    if io.test_consumed(port) {
                        *separated += u8::from(input.canonical_parts(port).is_some());
                        *canonical += u8::from(input.input(port).is_some());
                    }
                }
                result
            }
            Self::Sink {
                schema,
                expected,
                separated,
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
                if *delay > 0 {
                    *delay -= 1;
                    io.exhaust_fuel();
                    return StepOutcome::Yield;
                }
                if *separated {
                    let parts = input.canonical_parts(PortId(0)).unwrap();
                    assert_eq!(parts.schema, *schema);
                    assert_eq!(parts.node, &expected[schema.len()..]);
                    assert!(input.input(PortId(0)).is_none());
                } else {
                    assert_eq!(input.input(PortId(0)), Some(*expected));
                    assert!(input.canonical_parts(PortId(0)).is_none());
                }
                assert_eq!(
                    io.input(PortId(0)).unwrap().byte_len as usize,
                    expected.len()
                );
                io.consume(PortId(0)).unwrap();
                *staged = true;
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
            Self::Zip { back, .. } => <FlowZipBack as StepBack<2>>::prepared_output(back, port),
            _ => None,
        }
    }
    fn step_committed(&mut self) {
        match self {
            Self::Source { count, staged, .. } | Self::Sink { count, staged, .. } => {
                *count += u8::from(*staged);
                *staged = false;
            }
            Self::Zip { back, .. } => <FlowZipBack as StepBack<2>>::step_committed(back),
        }
    }
}
#[test]
fn actual_scheduler_pairs_mixed_inputs_with_pressure_and_canonical_fallback() {
    for (fallback, feedback, flush) in [
        (false, false, false),
        (true, false, false),
        (false, true, false),
        (false, false, true),
    ] {
        let (ty, schema, bytes) = fixture();
        let contract = CheckedValueContract::new(
            ty.profile().unwrap().value_kind().clone(),
            bytes[0].len() as u32,
            vec![],
        )
        .unwrap();
        let mut encoder = PreparedTypedTuplePairEncoder::new(
            ty.clone(),
            contract.maximum_bytes,
            ty.clone(),
            contract.maximum_bytes,
        )
        .unwrap();
        let expected = encoder.encode(&bytes[0], &bytes[1]).unwrap().to_vec();
        let output_schema = encoder.value_type().canonical_bytes().unwrap();
        let input_validator =
            BorrowedStructuredValueValidator::new(schema, bytes[0].len()).unwrap();
        let output_validator =
            BorrowedStructuredValueValidator::new(&output_schema, expected.len()).unwrap();
        let store = SchemaValueStore::try_new(
            FixedValueStore::<12, 2048>::new(24576).unwrap(),
            &[(input_validator, 6), (output_validator, 6)],
            (bytes[0].len() * 6 + expected.len() * 6) as u32,
            32768,
        )
        .unwrap();
        let mut routes = FixedRoutes::<5, 3>::new(2);
        for (source, sink, port, cord) in [(0, 2, 0, 0), (1, 2, 1, 1), (2, 3, 0, 2)] {
            routes
                .install(
                    NodeId(source),
                    PortId(0),
                    RouteRange {
                        start: cord,
                        len: 1,
                    },
                    &[RouteTarget {
                        cord: CordId(cord),
                        sink: conduit_kernel::CordEndpoint::local(NodeId(sink), PortId(port)),
                    }],
                )
                .unwrap();
        }
        routes.seal().unwrap();
        let mut back = if feedback {
            FlowZipBack::prepare_typed_feedback(&contract, ty.clone(), &contract, ty)
        } else {
            FlowZipBack::prepare_typed_finite(&contract, ty.clone(), &contract, ty)
        }
        .unwrap();
        if flush {
            // Enter with one committed pair, then expose the source closes.
            // This exercises schema-backed finite terminal emission in the scheduler.
            for side in 0..2 {
                let mut refs = [None; 2];
                let mut inputs = [None; 2];
                refs[side] = Some(conduit_kernel::ValueRef {
                    slot: side as u16,
                    generation: 1,
                    byte_len: bytes[side].len() as u32,
                });
                inputs[side] = Some(bytes[side].as_slice());
                let mut io = StepIo::test_frame(refs, [false; 2], [None; 2], None, 16);
                assert_eq!(
                    back.step(&mut io, &StepInputBytes::test_frame(inputs, None)),
                    StepOutcome::Progress
                );
                <FlowZipBack as StepBack<2>>::step_committed(&mut back);
            }
        }
        let drivers = [
            Back::Source {
                schema,
                bytes: &bytes[0],
                separated: true,
                count: if flush { 2 } else { 0 },
                close_delay: if flush { 0 } else { 24 },
                staged: false,
            },
            Back::Source {
                schema,
                bytes: &bytes[1],
                separated: false,
                count: if flush { 2 } else { 0 },
                close_delay: if flush { 0 } else { 24 },
                staged: false,
            },
            Back::Zip {
                back,
                separated: 0,
                canonical: 0,
            },
            Back::Sink {
                schema: &output_schema,
                expected: &expected,
                separated: !fallback,
                delay: 12,
                count: 0,
                staged: false,
            },
        ];
        let endpoints = [(0, 2, 0), (1, 2, 1), (2, 3, 0)];
        let mut kernel = FixedScheduler::<_, _, _, 4, 3, 2, 3, 5, 3, 4, 1>::new(
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
                        byte_capacity: if n == 2 {
                            expected.len() as u32
                        } else {
                            contract.maximum_bytes
                        },
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
        let Back::Zip {
            separated,
            canonical,
            ..
        } = &kernel.drivers()[2]
        else {
            unreachable!()
        };
        assert_eq!(
            (*separated, *canonical),
            if flush { (0, 0) } else { (2, 2) }
        );
        let Back::Sink { count, .. } = &kernel.drivers()[3] else {
            unreachable!()
        };
        assert_eq!(
            *count,
            if flush { 1 } else { 2 },
            "fallback={fallback}, feedback={feedback}, flush={flush}"
        );
        assert_eq!(kernel.values().used_items(), 0);
        assert_eq!(kernel.values().used_bytes(), 0);
    }
}

#[test]
fn pair_waits_for_complete_output_allowance_before_payload_or_close_commit() {
    for close in [false, true] {
        let ty = StructuredInfoType::leaf(kind_id("value/count")).unwrap();
        let contract = CheckedValueContract::new(kind_id("value/count"), 8, vec![]).unwrap();
        let mut back =
            FlowZipBack::prepare_typed_finite(&contract, ty.clone(), &contract, ty).unwrap();
        let bytes = [7_u64.to_le_bytes(), 8_u64.to_le_bytes()];
        for side in 0..2 {
            let mut references = [None; 2];
            let mut inputs = [None; 2];
            references[side] = Some(conduit_kernel::ValueRef {
                slot: side as u16,
                generation: 1,
                byte_len: 8,
            });
            inputs[side] = Some(bytes[side].as_slice());
            let mut io = StepIo::test_frame(references, [false; 2], [None; 2], None, 16);
            assert_eq!(
                back.step(&mut io, &StepInputBytes::test_frame(inputs, None)),
                StepOutcome::Progress
            );
            <FlowZipBack as StepBack<2>>::step_committed(&mut back);
        }
        let length = back.encode_pair().unwrap() as u32;
        let allocations = crate::test_support::allocation::allocations_during(|| {
            for _ in 0..16 {
                let mut io = StepIo::test_frame(
                    [None; 2],
                    [close, false],
                    [Some(length - 1), None],
                    None,
                    16,
                );
                assert_eq!(
                    back.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
                    StepOutcome::Await
                );
                assert!(!io.test_consumed_closed(PortId(0)));
                assert_eq!((back.left_len, back.right_len), (Some(8), Some(8)));
                assert!(!back.output_staged && !back.finish_after_commit);
            }
            let mut io =
                StepIo::test_frame([None; 2], [close, false], [Some(length), None], None, 16);
            assert_eq!(
                back.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
                StepOutcome::Progress
            );
            assert_eq!(io.test_consumed_closed(PortId(0)), close);
            <FlowZipBack as StepBack<2>>::step_committed(&mut back);
            assert_eq!((back.left_len, back.right_len), (None, None));
        });
        assert_eq!(allocations, 0);
    }
}
