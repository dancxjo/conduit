//! Exact canonical bytes borrowed by a bounded Step.

use conduit_kernel::scheduler::{
    CordCapacity, CordSpec, FixedScheduler, NodeSpec, SchedulerError, StepInputBytes, StepIo,
    StepOperation, StepOutcome,
};
#[cfg(feature = "alloc")]
use conduit_kernel::HostedValueStore;
use conduit_kernel::{
    CordId, FixedRoutes, FixedSignLog, FixedValueStore, NodeId, PortId, RouteRange, RouteTarget,
    ValueRef, ValueStorage,
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

enum ProbeBack {
    Source {
        value: ValueRef,
        sent: bool,
    },
    Sink {
        kind: DecodeKind,
        observed: Observed,
    },
}

impl StepOperation<PORTS> for ProbeBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        match self {
            Self::Source { value, sent: false } if io.output_ready(PortId(0)) => {
                io.send(PortId(0), *value).unwrap();
                if let Self::Source { sent, .. } = self {
                    *sent = true;
                }
                StepOutcome::Progress
            }
            Self::Source { sent: true, .. } => StepOutcome::Complete,
            Self::Source { .. } => StepOutcome::Await,
            Self::Sink { kind, observed } => {
                if let Some(value) = io.input(PortId(0)) {
                    let canonical = input_bytes.input(PortId(0)).expect("admitted input bytes");
                    assert_eq!(value.byte_len as usize, canonical.len());
                    *observed = match kind {
                        DecodeKind::Bool => Observed::Bool(match canonical {
                            [0] => false,
                            [1] => true,
                            _ => panic!("kernel supplied noncanonical bool fixture bytes"),
                        }),
                        DecodeKind::Scalar => Observed::Scalar(i64::from_le_bytes(
                            canonical.try_into().expect("exact scalar fixture bytes"),
                        )),
                    };
                    io.consume(PortId(0)).unwrap();
                    StepOutcome::Progress
                } else if io.input_closed(PortId(0)) {
                    io.consume_closed(PortId(0)).unwrap();
                    StepOutcome::Complete
                } else {
                    StepOutcome::Await
                }
            }
        }
    }
}

#[test]
fn fixed_and_hosted_stores_expose_the_same_exact_step_input_bytes() {
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
fn stale_identity_fails_before_a_step_payload_decision() {
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
            pressure_policy: Default::default(),
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
        ProbeBack::Source { value, sent: false },
        ProbeBack::Sink {
            kind,
            observed: Observed::None,
        },
    ];
    let sign_bytes = (SIGN_EVENTS * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32;
    let signs = FixedSignLog::<SIGN_EVENTS>::new(sign_bytes).unwrap();
    let mut scheduler = FixedScheduler::<_, _, _, 2, 1, PORTS, 1, 2, 1>::new(
        node_specs, cord_specs, routes, drivers, values, signs,
    )?;
    scheduler.run(16)?;
    let ProbeBack::Sink { observed, .. } = scheduler.drivers()[1] else {
        panic!("sink Back identity changed");
    };
    Ok(observed)
}
