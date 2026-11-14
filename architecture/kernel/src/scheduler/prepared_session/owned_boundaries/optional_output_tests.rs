//! Optional Value outputs preserve each Cord's real sequence across rearm.
use super::*;
use crate::scheduler::{CordCapacity, NodeSpec, StepBack, StepInputBytes, StepIo, StepOutcome};
use crate::{FixedRoutes, FixedSignLog, FixedValueStore, PortId, RouteRange, RouteTarget};

struct Select {
    emit: [bool; 2],
    finished: bool,
    staged: bool,
}
impl StepBack<2> for Select {
    fn step(&mut self, io: &mut StepIo<2>, _: &StepInputBytes<'_, 2>) -> StepOutcome {
        if self.finished {
            return StepOutcome::Complete;
        }
        let Some(value) = io.input(PortId(0)) else {
            return StepOutcome::Await;
        };
        for (index, emit) in self.emit.iter().enumerate() {
            if *emit && !io.output_ready(PortId(index as u16)) {
                return StepOutcome::Await;
            }
        }
        for (index, emit) in self.emit.iter().enumerate() {
            if *emit {
                io.send(PortId(index as u16), value).unwrap();
            }
        }
        io.consume(PortId(0)).unwrap();
        self.staged = true;
        StepOutcome::Progress
    }
    fn step_committed(&mut self) {
        if self.staged {
            self.finished = true;
            self.staged = false;
        }
    }
}
impl ReusableStepBack<2> for Select {
    type Next = [bool; 2];
    fn preflight_next(&self, _: &Self::Next, _: &dyn ValueStorage) -> bool {
        self.finished && !self.staged
    }
    fn begin_next(&mut self, next: Self::Next) {
        self.emit = next;
        self.finished = false;
    }
}
type Log = FixedSignLog<96>;
type Kernel = FixedScheduler<Select, FixedValueStore<4, 16>, Log, 1, 3, 2, 3, 2, 2>;
type Session = PreparedKernelSession<Select, FixedValueStore<4, 16>, Log, 1, 3, 2, 3, 2, 2>;
const INPUT: OwnedValueBoundary = OwnedValueBoundary {
    cord: CordId(0),
    endpoint: RemoteEndpointId(0),
};
const OUTPUTS: [OwnedValueBoundary; 2] = [
    OwnedValueBoundary {
        cord: CordId(1),
        endpoint: RemoteEndpointId(1),
    },
    OwnedValueBoundary {
        cord: CordId(2),
        endpoint: RemoteEndpointId(2),
    },
];
fn log() -> Log {
    Log::new_with_remote_storage(
        96 * core::mem::size_of::<crate::KernelEvent>() as u32,
        32,
        crate::remote_sign_storage_bytes(32).unwrap(),
    )
    .unwrap()
}
fn kernel(emit: [bool; 2]) -> Box<Kernel> {
    let mut routes = FixedRoutes::new(2);
    for (index, output) in OUTPUTS.iter().enumerate() {
        routes
            .install(
                NodeId(0),
                PortId(index as u16),
                RouteRange {
                    start: index as u16,
                    len: 1,
                },
                &[RouteTarget {
                    cord: output.cord,
                    sink: CordEndpoint::Remote(output.endpoint),
                }],
            )
            .unwrap();
    }
    routes.seal().unwrap();
    let capacity = |slot_start| CordCapacity {
        slot_start,
        item_capacity: 1,
        byte_capacity: 16,
        pressure_policy: Default::default(),
    };
    Box::new(
        Kernel::new(
            [NodeSpec {
                input_cords: [Some(INPUT.cord), None],
                maximum_step_fuel: 8,
            }],
            [
                CordSpec::remote_ingress(
                    INPUT.cord,
                    INPUT.endpoint,
                    (NodeId(0), PortId(0)),
                    capacity(0),
                ),
                CordSpec::remote_egress(
                    OUTPUTS[0].cord,
                    (NodeId(0), PortId(0)),
                    OUTPUTS[0].endpoint,
                    capacity(1),
                ),
                CordSpec::remote_egress(
                    OUTPUTS[1].cord,
                    (NodeId(0), PortId(1)),
                    OUTPUTS[1].endpoint,
                    capacity(2),
                ),
            ],
            routes,
            [Select {
                emit,
                finished: false,
                staged: false,
            }],
            FixedValueStore::new(64).unwrap(),
            log(),
        )
        .unwrap(),
    )
}
fn session(emit: [bool; 2], optional: &[OwnedValueBoundary]) -> Session {
    Session::new_with_optional_owned_outputs(
        kernel(emit),
        &[INPUT, OUTPUTS[0], OUTPUTS[1]],
        optional,
        24,
    )
    .unwrap_or_else(|_| panic!("exact owned profile"))
}
fn invoke(s: &mut Session, input_sequence: u64, expected: [Option<u64>; 2]) {
    s.admit_owned_input(INPUT.endpoint, INPUT.cord, input_sequence, &[7])
        .unwrap();
    s.close_owned_input(INPUT.endpoint, INPUT.cord).unwrap();
    let mut seen = [false; 2];
    for _ in 0..12 {
        let status = s.step().unwrap();
        if status == SchedulerStatus::Drained {
            assert_eq!(seen, expected.map(|sequence| sequence.is_some()));
            return;
        }
        for (index, output) in OUTPUTS.iter().enumerate() {
            if let Some(value) = s.owned_output(output.endpoint, output.cord).unwrap() {
                assert_eq!(Some(value.sequence), expected[index]);
                assert!(!seen[index]);
                seen[index] = true;
                s.accept_owned_output(output.endpoint, output.cord, value.sequence)
                    .unwrap();
            }
        }
    }
    panic!("bounded invocation did not drain");
}
#[test]
fn mixed_empty_and_emitted_outputs_keep_independent_sequences_and_evidence() {
    let mut s = session([false, true], &OUTPUTS);
    let address = s.kernel() as *const _ as usize;
    invoke(&mut s, 0, [None, Some(0)]);
    let evidence = s.kernel().signs().events().collect::<alloc::vec::Vec<_>>();
    let previous = s
        .begin_next([[true, false]], log())
        .unwrap_or_else(|_| panic!("normal empty closure"));
    assert_eq!(
        previous.signs.events().collect::<alloc::vec::Vec<_>>(),
        evidence
    );
    invoke(&mut s, 1, [Some(0), None]);
    s.begin_next([[false, false]], log())
        .unwrap_or_else(|_| panic!("mixed closure"));
    invoke(&mut s, 2, [None, None]);
    s.begin_next([[true, true]], log())
        .unwrap_or_else(|_| panic!("both empty"));
    invoke(&mut s, 3, [Some(1), Some(1)]);
    assert_eq!(address, s.kernel() as *const _ as usize);
    assert_eq!(s.kernel().values().used_items(), 0);
}
#[test]
fn optionality_is_explicit_and_never_applies_to_input_or_wrong_endpoint() {
    for optional in [
        alloc::vec![INPUT],
        alloc::vec![OUTPUTS[0], OUTPUTS[0]],
        alloc::vec![OwnedValueBoundary {
            endpoint: RemoteEndpointId(9),
            ..OUTPUTS[0]
        }],
    ] {
        assert!(matches!(
            Session::new_with_optional_owned_outputs(
                kernel([false, true]),
                &[INPUT, OUTPUTS[0], OUTPUTS[1]],
                &optional,
                24
            ),
            Err((SessionRefusal::InvalidOwnedBoundary, _))
        ));
    }
    let mut required = session([false, true], &[]);
    invoke(&mut required, 0, [None, Some(0)]);
    assert!(matches!(
        required.begin_next([[true, true]], log()),
        Err(NextInvocationRefused {
            reason: SessionRefusal::OutstandingOwnership,
            ..
        })
    ));
}
#[test]
fn offered_unaccepted_output_cancellation_and_stale_ack_cannot_rearm() {
    let mut s = session([true, false], &OUTPUTS);
    s.admit_owned_input(INPUT.endpoint, INPUT.cord, 0, &[7])
        .unwrap();
    s.close_owned_input(INPUT.endpoint, INPUT.cord).unwrap();
    s.step().unwrap();
    let output = s
        .owned_output(OUTPUTS[0].endpoint, OUTPUTS[0].cord)
        .unwrap()
        .unwrap();
    assert!(s.begin_next([[false, false]], log()).is_err());
    assert!(s.kernel().values().get(output.value).is_ok());
    s.cancel().unwrap();
    assert!(s.begin_next([[true, true]], log()).is_err());
    let mut s = session([true, false], &OUTPUTS);
    invoke(&mut s, 0, [Some(0), None]);
    s.begin_next([[true, false]], log())
        .unwrap_or_else(|_| panic!("drained"));
    assert_eq!(
        s.accept_owned_output(OUTPUTS[0].endpoint, OUTPUTS[0].cord, 0),
        Err(SchedulerError::RemoteSequenceRejected)
    );
    assert!(s.begin_next([[false, false]], log()).is_err());
}

#[test]
fn optional_tracking_is_charged_only_for_selected_sessions_and_not_inline() {
    type WideSession =
        PreparedKernelSession<Select, FixedValueStore<4, 16>, Log, 1, 128, 2, 3, 2, 2>;
    assert_eq!(
        core::mem::size_of::<Session>(),
        core::mem::size_of::<WideSession>()
    );
    let ordinary = session([true, true], &[]);
    assert_eq!(ordinary.optional_output_storage_bytes(), 0);
    let mut optional = session([false, true], &OUTPUTS);
    let charged = optional.optional_output_storage_bytes();
    assert_eq!(charged, Session::optional_output_storage_requirement());
    assert!(charged > 0);
    invoke(&mut optional, 0, [None, Some(0)]);
    let _old = optional
        .begin_next([[true, false]], log())
        .unwrap_or_else(|_| panic!("normally drained optional session"));
    assert_eq!(optional.optional_output_storage_bytes(), charged);
    invoke(&mut optional, 1, [Some(0), None]);
}

#[test]
fn exact_optional_bind_consumes_preallocated_tracking_without_reallocation() {
    let storage = super::super::OptionalOutputStorage::<3>::prepare();
    let address = storage.backing_identity();
    let mut s = Session::new_with_preallocated_optional_outputs(
        kernel([false, true]),
        &[INPUT, OUTPUTS[0], OUTPUTS[1]],
        &OUTPUTS,
        24,
        storage,
    )
    .unwrap_or_else(|_| panic!("exact preallocated binding"));
    assert_eq!(
        (&**s.optional_outputs.as_ref().unwrap() as *const _) as usize,
        address
    );
    invoke(&mut s, 0, [None, Some(0)]);
    let _evidence = s
        .begin_next([[true, false]], log())
        .unwrap_or_else(|_| panic!("drained"));
    assert_eq!(
        (&**s.optional_outputs.as_ref().unwrap() as *const _) as usize,
        address
    );
    invoke(&mut s, 1, [Some(0), None]);
    assert!(Session::new_with_preallocated_optional_outputs(
        kernel([true, true]),
        &[INPUT, OUTPUTS[0], OUTPUTS[1]],
        &[],
        24,
        super::super::OptionalOutputStorage::prepare()
    )
    .is_err());
    assert!(Session::new_with_preallocated_optional_outputs(
        kernel([true, true]),
        &[INPUT, OUTPUTS[0], OUTPUTS[1]],
        &[INPUT],
        24,
        super::super::OptionalOutputStorage::prepare()
    )
    .is_err());
}
