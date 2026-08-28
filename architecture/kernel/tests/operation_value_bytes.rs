use conduit_kernel::scheduler::{
    CordCapacity, CordSpec, FixedScheduler, NodeSpec, OperationDriver, SchedulerError,
};
#[cfg(feature = "alloc")]
use conduit_kernel::HostedValueStore;
use conduit_kernel::{
    CordId, FixedRoutes, FixedSignLog, FixedValueStore, NodeId, Operation, OperationAction,
    OperationInput, PortId, RouteRange, RouteTarget, ValueRef, ValueStorage,
};

const PORTS: usize = 1;
const SIGN_EVENTS: usize = 64;

#[derive(Clone, Copy)]
enum DecodeKind {
    Bool,
    Scalar,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Observed {
    None,
    Bool(bool),
    Scalar(i64),
}

enum ProbeOperation {
    Source {
        value: ValueRef,
    },
    Sink {
        kind: DecodeKind,
        observed: Observed,
    },
}

impl Operation for ProbeOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Source { value } => OperationAction::Emit {
                port: PortId(0),
                value: *value,
            },
            Self::Sink { .. } => OperationAction::Await,
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match (self, input) {
            (Self::Sink { .. }, OperationInput::Closed { port: PortId(0) }) => {
                OperationAction::Complete
            }
            _ => panic!("probe received an invalid opaque input"),
        }
    }

    fn resume_value(&mut self, port: PortId, value: ValueRef, canonical: &[u8]) -> OperationAction {
        let Self::Sink { kind, observed } = self else {
            panic!("source cannot receive a value");
        };
        assert_eq!(port, PortId(0));
        assert_eq!(value.byte_len as usize, canonical.len());
        *observed = match kind {
            DecodeKind::Bool => Observed::Bool(match canonical {
                [0] => false,
                [1] => true,
                _ => panic!("kernel supplied noncanonical bool fixture bytes"),
            }),
            DecodeKind::Scalar => Observed::Scalar(i64::from_le_bytes(
                canonical
                    .try_into()
                    .expect("kernel supplied exact scalar fixture bytes"),
            )),
        };
        OperationAction::Await
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Source { .. } => OperationAction::Complete,
            Self::Sink { .. } => OperationAction::Await,
        }
    }
}

#[test]
fn fixed_and_hosted_stores_expose_the_same_exact_resumed_bytes() {
    let fixed_bool = run_case(
        FixedValueStore::<4, 8>::new(24).unwrap(),
        &[1],
        DecodeKind::Bool,
    )
    .unwrap();
    assert_eq!(fixed_bool, Observed::Bool(true));
    #[cfg(feature = "alloc")]
    {
        let hosted_bool = run_case(
            HostedValueStore::new(4, 8, 24).unwrap(),
            &[1],
            DecodeKind::Bool,
        )
        .unwrap();
        assert_eq!(hosted_bool, fixed_bool);
    }

    let scalar = -9_223_372_036_i64;
    let fixed_scalar = run_case(
        FixedValueStore::<4, 8>::new(24).unwrap(),
        &scalar.to_le_bytes(),
        DecodeKind::Scalar,
    )
    .unwrap();
    assert_eq!(fixed_scalar, Observed::Scalar(scalar));
    #[cfg(feature = "alloc")]
    {
        let hosted_scalar = run_case(
            HostedValueStore::new(4, 8, 24).unwrap(),
            &scalar.to_le_bytes(),
            DecodeKind::Scalar,
        )
        .unwrap();
        assert_eq!(hosted_scalar, fixed_scalar);
    }
}

#[test]
fn stale_identity_fails_before_a_payload_decision() {
    let mut values = FixedValueStore::<4, 8>::new(24).unwrap();
    let stale = values.store(&[1]).unwrap();
    values.release(stale).unwrap();
    assert_eq!(
        run_with_value(values, stale, DecodeKind::Bool),
        Err(SchedulerError::Storage(
            conduit_kernel::StorageError::StaleReference
        ))
    );
}

#[test]
fn the_default_hook_preserves_opaque_operation_behavior() {
    struct Opaque {
        seen: Option<(PortId, ValueRef)>,
    }

    impl Operation for Opaque {
        fn start(&mut self) -> OperationAction {
            OperationAction::Await
        }

        fn resume(&mut self, input: OperationInput) -> OperationAction {
            let OperationInput::Value { port, value } = input else {
                panic!("opaque fixture expects a value");
            };
            self.seen = Some((port, value));
            OperationAction::Await
        }
    }

    let value = ValueRef {
        slot: 7,
        generation: 3,
        byte_len: 1,
    };
    let mut opaque = Opaque { seen: None };
    assert_eq!(
        opaque.resume_value(PortId(2), value, &[1]),
        OperationAction::Await
    );
    assert_eq!(opaque.seen, Some((PortId(2), value)));
}

fn run_case<S: ValueStorage>(
    mut values: S,
    bytes: &[u8],
    kind: DecodeKind,
) -> Result<Observed, SchedulerError> {
    let value = values.store(bytes)?;
    run_with_value(values, value, kind)
}

fn run_with_value<S: ValueStorage>(
    values: S,
    value: ValueRef,
    kind: DecodeKind,
) -> Result<Observed, SchedulerError> {
    let node_specs = [
        NodeSpec {
            input_cords: [None],
            maximum_step_work: 2,
        },
        NodeSpec {
            input_cords: [Some(CordId(0))],
            maximum_step_work: 2,
        },
    ];
    let cord_specs = [CordSpec::local(
        CordId(0),
        (NodeId(0), PortId(0)),
        (NodeId(1), PortId(0)),
        CordCapacity {
            slot_start: 0,
            item_capacity: 1,
            byte_capacity: 8,
        },
    )];
    let mut routes = FixedRoutes::<2, 1>::new(PORTS as u16);
    routes.install(
        NodeId(0),
        PortId(0),
        RouteRange { start: 0, len: 1 },
        &[RouteTarget {
            cord: CordId(0),
            sink: conduit_kernel::CordEndpoint::local(NodeId(1), PortId(0)),
        }],
    )?;
    routes.seal()?;
    let drivers = [
        OperationDriver::new(ProbeOperation::Source { value })?,
        OperationDriver::new(ProbeOperation::Sink {
            kind,
            observed: Observed::None,
        })?,
    ];
    let sign_bytes = (SIGN_EVENTS * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32;
    let signs = FixedSignLog::<SIGN_EVENTS>::new(sign_bytes).unwrap();
    let mut scheduler = FixedScheduler::<_, _, _, 2, 1, PORTS, 1, 2, 1>::new(
        node_specs, cord_specs, routes, drivers, values, signs,
    )?;
    scheduler.run(16)?;
    let ProbeOperation::Sink { observed, .. } = scheduler.drivers()[1].operation() else {
        panic!("sink driver identity changed");
    };
    Ok(*observed)
}
