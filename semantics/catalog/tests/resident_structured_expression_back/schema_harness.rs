use conduit_core::BorrowedStructuredValueValidator;
use conduit_kernel::{
    scheduler::*, CordEndpoint, CordId, FixedRoutes, FixedSignLog, FixedValueStore, NodeId, PortId,
    RouteRange, RouteTarget, ValueRef, ValueStorage,
};
use conduit_plan_lowering::schema_value_store::SchemaValueStore;
enum Back {
    Source {
        value: ValueRef,
        schema: &'static [u8],
        sent: bool,
        staged: bool,
    },
    Expression(image::Back),
    Sink {
        complete: bool,
        staged: bool,
    },
}
impl StepBack<1> for Back {
    fn canonical_input_schema(&self, port: PortId) -> Option<&[u8]> {
        match self {
            Self::Expression(b) => <image::Back as StepBack<1>>::canonical_input_schema(b, port),
            Self::Sink { .. } => SEPARATED.then(image::output_schema),
            _ => None,
        }
    }
    fn canonical_output_schema(&self, port: PortId) -> Option<&[u8]> {
        match self {
            Self::Source { schema, .. } => SEPARATED.then_some(schema),
            Self::Expression(b) => <image::Back as StepBack<1>>::canonical_output_schema(b, port),
            _ => None,
        }
    }
    fn step(&mut self, io: &mut StepIo<1>, input: &StepInputBytes<'_, 1>) -> StepOutcome {
        match self {
            Self::Source {
                value,
                sent,
                staged,
                ..
            } => {
                *staged = false;
                if *sent {
                    return StepOutcome::Complete;
                }
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send(PortId(0), *value).unwrap();
                *staged = true;
                StepOutcome::Progress
            }
            Self::Expression(b) => b.step(io, input),
            Self::Sink { complete, staged } => {
                *staged = false;
                if *complete {
                    return StepOutcome::Complete;
                }
                if io.input(PortId(0)).is_none() {
                    return StepOutcome::Await;
                }
                if SEPARATED {
                    let parts = input.canonical_parts(PortId(0)).unwrap();
                    assert_eq!(parts.schema, image::output_schema());
                    assert_eq!(parts.schema, &EXPECTED[..parts.schema.len()]);
                    assert_eq!(parts.node, &EXPECTED[parts.schema.len()..]);
                    assert!(input.input(PortId(0)).is_none());
                } else {
                    assert_eq!(input.input(PortId(0)), Some(EXPECTED));
                }
                io.consume(PortId(0)).unwrap();
                *staged = true;
                StepOutcome::Progress
            }
        }
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        match self {
            Self::Expression(b) => <image::Back as StepBack<1>>::prepared_output(b, port),
            _ => None,
        }
    }
    fn step_committed(&mut self) {
        match self {
            Self::Source { sent, staged, .. } => {
                *sent |= *staged;
                *staged = false;
            }
            Self::Expression(b) => <image::Back as StepBack<1>>::step_committed(b),
            Self::Sink { complete, staged } => {
                *complete |= *staged;
                *staged = false;
            }
        }
    }
}
fn exercise(input: &[u8], invalid: bool) {
    let schema = image::input_schema();
    let validator = BorrowedStructuredValueValidator::new(schema, input.len()).unwrap();
    // Core shape validation deliberately supplies no Native predicate witness.
    let checked = validator.validate(input).unwrap();
    let canonical = FixedValueStore::<2, 2048>::new(if SEPARATED { 1 } else { 4096 }).unwrap();
    let output_validator =
        BorrowedStructuredValueValidator::new(image::output_schema(), EXPECTED.len()).unwrap();
    let mut values = SchemaValueStore::try_new(
        canonical,
        &[(validator, 1), (output_validator, 1)],
        (input.len() + EXPECTED.len()) as u32,
        4096,
    )
    .unwrap();
    let value = if SEPARATED {
        values.store_structured(checked).unwrap()
    } else {
        values.store(input).unwrap()
    };
    assert_eq!(values.get(value).is_err(), SEPARATED);
    let expression = if SEPARATED {
        image::parts_back()
    } else {
        image::canonical_back()
    };
    let fuel = expression.required_step_fuel();
    let mut routes = FixedRoutes::<3, 2>::new(1);
    for n in 0..2 {
        routes
            .install(
                NodeId(n),
                PortId(0),
                RouteRange { start: n, len: 1 },
                &[RouteTarget {
                    cord: CordId(n),
                    sink: CordEndpoint::local(NodeId(n + 1), PortId(0)),
                }],
            )
            .unwrap();
    }
    routes.seal().unwrap();
    let signs = FixedSignLog::<128>::new(
        (128 * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32,
    )
    .unwrap();
    let mut kernel = FixedScheduler::<_, _, _, 3, 2, 1, 2, 3, 2>::new(
        core::array::from_fn(|n| NodeSpec {
            input_cords: [if n == 0 {
                None
            } else {
                Some(CordId((n - 1) as u16))
            }],
            maximum_step_fuel: if n == 1 { fuel } else { 3 },
        }),
        core::array::from_fn(|n| {
            CordSpec::local(
                CordId(n as u16),
                (NodeId(n as u16), PortId(0)),
                (NodeId(n as u16 + 1), PortId(0)),
                CordCapacity {
                    slot_start: n as u16,
                    item_capacity: 1,
                    byte_capacity: if n == 0 {
                        input.len() as u32
                    } else {
                        EXPECTED.len() as u32
                    },
                    pressure_policy: Default::default(),
                },
            )
        }),
        routes,
        [
            Back::Source {
                value,
                schema,
                sent: false,
                staged: false,
            },
            Back::Expression(expression),
            Back::Sink {
                complete: false,
                staged: false,
            },
        ],
        values,
        signs,
    )
    .unwrap();
    let (result, cost) = probe::observe(|| kernel.run(32));
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
    let Back::Expression(back) = &kernel.drivers()[1] else {
        panic!("expression")
    };
    if invalid {
        assert!(matches!(
            back.last_evaluation_refusal(),
            Some(conduit_plot::StaticStructuredRefusal::Input(_))
        ));
        assert!(matches!(
            kernel.drivers()[2],
            Back::Sink {
                complete: false,
                ..
            }
        ));
    } else {
        result.unwrap();
        assert!(back.last_evaluation_refusal().is_none());
        assert!(matches!(
            kernel.drivers()[2],
            Back::Sink { complete: true, .. }
        ));
        assert_eq!(kernel.values().used_items(), 0);
    }
}
fn main() {
    exercise(INPUT, false);
    exercise(BAD, true);
}
