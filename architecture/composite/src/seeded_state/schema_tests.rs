//! Retained State custody through real schema queues and an admitted shared workspace.
use super::*;
use alloc::{boxed::Box, vec, vec::Vec};
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
        kind_id("test/state-schema-custody"),
        vec![
            StructuredFieldType::new("number", number.clone()).unwrap(),
            StructuredFieldType::new("terminal", boolean.clone()).unwrap(),
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
                    "terminal",
                    StructuredInfoValue::leaf(boolean.clone(), vec![u8::from(n == 2)]).unwrap(),
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
    State {
        back: SeededStateBack,
        separated: u8,
        canonical: u8,
        blocked: u16,
    },
    Sink {
        schema: &'a [u8],
        expected: [&'a [u8]; 2],
        second_separated: bool,
        separated: bool,
        delay: u8,
        count: u8,
        staged: bool,
    },
}
impl StepBack<2> for Back<'_> {
    fn canonical_input_schema(&self, port: PortId) -> Option<&[u8]> {
        match self {
            Self::State { back, .. } => {
                <SeededStateBack as StepBack<2>>::canonical_input_schema(back, port)
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
            Self::State { back, .. } => {
                <SeededStateBack as StepBack<2>>::canonical_output_schema(back, port)
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
            Self::State { back, .. } => back.terminal_transductions(),
            _ => [None; 2],
        }
    }
    fn workspace_requirements(&self) -> Option<StepWorkspaceRequirements> {
        match self {
            Self::State { back, .. } => {
                <SeededStateBack as StepBack<2>>::workspace_requirements(back)
            }
            _ => None,
        }
    }
    fn step_with_workspace(
        &mut self,
        io: &mut StepIo<2>,
        input: &StepInputBytes<'_, 2>,
        workspace: &mut StepWorkspace<'_>,
    ) -> StepOutcome {
        self.step_inner(io, input, Some(workspace))
    }
    fn step(&mut self, io: &mut StepIo<2>, input: &StepInputBytes<'_, 2>) -> StepOutcome {
        self.step_inner(io, input, None)
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        match self {
            Self::Source {
                bytes,
                staged: true,
                ..
            } if port == PortId(0) => Some(bytes),
            Self::State { back, .. } => {
                <SeededStateBack as StepBack<2>>::prepared_output(back, port)
            }
            _ => None,
        }
    }
    fn step_committed(&mut self) {
        match self {
            Self::Source { count, staged, .. } | Self::Sink { count, staged, .. } => {
                *count += u8::from(*staged);
                *staged = false;
            }
            Self::State { back, .. } => <SeededStateBack as StepBack<2>>::step_committed(back),
        }
    }
}
impl Back<'_> {
    fn step_inner(
        &mut self,
        io: &mut StepIo<2>,
        input: &StepInputBytes<'_, 2>,
        workspace: Option<&mut StepWorkspace<'_>>,
    ) -> StepOutcome {
        match self {
            Self::Source {
                bytes,
                count,
                close_delay,
                staged,
                ..
            } => {
                if *count == 1 {
                    // Keep producer closure independent of terminal State publication.
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
            Self::State {
                back,
                separated,
                canonical,
                blocked,
            } => {
                let held = back.held;
                let result = match workspace {
                    Some(workspace) => back.step_with_workspace(io, input, workspace),
                    None => back.step(io, input),
                };
                if result == StepOutcome::Await {
                    assert_eq!(back.held, held);
                    assert!(io.test_discards().iter().all(Option::is_none));
                    if io.input(PortId(1)).is_some() && !io.output_ready(PortId(0)) {
                        assert!(held.is_some());
                        *blocked += 1;
                    }
                }
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
                second_separated,
                separated,
                delay,
                count,
                staged,
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
                let expected = expected[usize::from(*count)];
                if *separated && (*count == 0 || *second_separated) {
                    let parts = input.canonical_parts(PortId(0)).unwrap();
                    assert_eq!(parts.schema, *schema);
                    assert_eq!(parts.node, &expected[schema.len()..]);
                    assert!(input.input(PortId(0)).is_none());
                } else {
                    assert_eq!(input.input(PortId(0)), Some(expected));
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
}

#[test]
fn admitted_workspace_preserves_state_replacement_terminal_and_canonical_fanout() {
    for (fallback, second_separated) in [(false, false), (true, false), (false, true)] {
        let (ty, schema, bytes) = fixture();
        let contract = CheckedValueContract::new(
            ty.profile().unwrap().value_kind().clone(),
            bytes[0].len() as u32,
            vec![],
        )
        .unwrap();
        let maximum = bytes[0].len() as u32;
        let validator = BorrowedStructuredValueValidator::new(schema, maximum as usize).unwrap();
        let store = SchemaValueStore::try_new(
            FixedValueStore::<12, 2048>::new(24576).unwrap(),
            &[(validator, 12)],
            maximum * 12,
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
        let back = SeededStateBack::prepare_until_static(&contract, &ty, schema).unwrap();
        let drivers = [
            Back::Source {
                schema,
                bytes: &bytes[0],
                separated: true,
                count: 0,
                close_delay: 24,
                staged: false,
            },
            Back::Source {
                schema,
                bytes: &bytes[1],
                separated: second_separated,
                count: 0,
                close_delay: 24,
                staged: false,
            },
            Back::State {
                back,
                separated: 0,
                canonical: 0,
                blocked: 0,
            },
            Back::Sink {
                schema,
                expected: [&bytes[0], &bytes[1]],
                second_separated,
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
        let insufficient = PreparedStepWorkspace::try_new(
            StepWorkspaceRequirements {
                output_bytes: maximum - 1,
                scratch_bytes: 0,
            },
            maximum as usize,
        )
        .unwrap();
        let (_, owner) = kernel
            .install_execution_workspace(insufficient)
            .unwrap_err();
        assert_eq!(owner.reserved_backing_bytes(), maximum as usize - 1);
        assert!(matches!(
            &kernel.drivers()[0],
            Back::Source { count: 0, .. }
        ));
        let funded = PreparedStepWorkspace::try_new(
            StepWorkspaceRequirements {
                output_bytes: maximum,
                scratch_bytes: 0,
            },
            maximum as usize,
        )
        .unwrap();
        kernel.install_execution_workspace(funded).unwrap();
        let allocations =
            crate::test_support::allocation::allocations_during(|| kernel.run(1024).unwrap());
        assert_eq!(allocations, 0);
        let Back::State {
            separated,
            canonical,
            blocked,
            back,
        } = &kernel.drivers()[2]
        else {
            unreachable!()
        };
        assert_eq!(
            (*separated, *canonical),
            if second_separated { (2, 0) } else { (1, 1) }
        );
        assert!(*blocked > 0);
        assert!(back.held.is_none() && back.staged.is_none());
        let Back::Sink { count, .. } = &kernel.drivers()[3] else {
            unreachable!()
        };
        assert_eq!(*count, 2);
        assert_eq!(kernel.values().used_items(), 0);
        assert_eq!(kernel.values().used_bytes(), 0);
    }
}
