use super::*;
use crate::scheduler::{CordCapacity, NodeSpec, StepBack, StepInputBytes, StepIo, StepOutcome};
use crate::{FixedRoutes, FixedSignLog, FixedValueStore, PortId, RouteRange, RouteTarget};
struct Echo {
    finished: bool,
    staged: bool,
}
impl StepBack<1> for Echo {
    fn step(&mut self, io: &mut StepIo<1>, _: &StepInputBytes<'_, 1>) -> StepOutcome {
        self.staged = false;
        if self.finished {
            return StepOutcome::Complete;
        }
        let Some(value) = io.input(PortId(0)) else {
            return StepOutcome::Await;
        };
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        io.send(PortId(0), value).unwrap();
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
impl ReusableStepBack<1> for Echo {
    type Next = ();
    fn preflight_next(&self, _: &(), _: &dyn ValueStorage) -> bool {
        self.finished && !self.staged
    }
    fn begin_next(&mut self, _: ()) {
        self.finished = false;
    }
}
type Log = FixedSignLog<64>;
type Kernel = FixedScheduler<Echo, FixedValueStore<2, 16>, Log, 1, 2, 1, 2, 1, 1>;
type Session = PreparedKernelSession<Echo, FixedValueStore<2, 16>, Log, 1, 2, 1, 2, 1, 1>;
const IN: OwnedValueBoundary = OwnedValueBoundary {
    cord: CordId(0),
    endpoint: RemoteEndpointId(0),
};
const OUT: OwnedValueBoundary = OwnedValueBoundary {
    cord: CordId(1),
    endpoint: RemoteEndpointId(1),
};
fn log(remote: u16) -> Log {
    Log::new_with_remote_storage(
        64 * core::mem::size_of::<crate::KernelEvent>() as u32,
        remote,
        crate::remote_sign_storage_bytes(remote).unwrap(),
    )
    .unwrap()
}
fn kernel() -> Box<Kernel> {
    let mut routes = FixedRoutes::new(1);
    routes
        .install(
            NodeId(0),
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: OUT.cord,
                sink: CordEndpoint::Remote(OUT.endpoint),
            }],
        )
        .unwrap();
    routes.seal().unwrap();
    let cap = |slot_start| CordCapacity {
        slot_start,
        item_capacity: 1,
        byte_capacity: 16,
        pressure_policy: Default::default(),
    };
    Box::new(
        Kernel::new(
            [NodeSpec {
                input_cords: [Some(IN.cord)],
                maximum_step_fuel: 8,
            }],
            [
                CordSpec::remote_ingress(IN.cord, IN.endpoint, (NodeId(0), PortId(0)), cap(0)),
                CordSpec::remote_egress(OUT.cord, (NodeId(0), PortId(0)), OUT.endpoint, cap(1)),
            ],
            routes,
            [Echo {
                finished: false,
                staged: false,
            }],
            FixedValueStore::new(32).unwrap(),
            log(32),
        )
        .unwrap(),
    )
}
fn session() -> Session {
    Session::new_with_owned_value_boundaries(kernel(), &[IN, OUT], 16)
        .unwrap_or_else(|_| panic!("exact local boundaries"))
}
fn invoke(s: &mut Session, sequence: u64, bytes: &[u8]) {
    assert_eq!(
        s.admit_owned_input(IN.endpoint, IN.cord, sequence, bytes)
            .unwrap(),
        RemoteIngressOutcome::Accepted { sequence }
    );
    s.close_owned_input(IN.endpoint, IN.cord).unwrap();
    let mut accepted = false;
    for _ in 0..8 {
        s.step().unwrap();
        if let Some(value) = s.owned_output(OUT.endpoint, OUT.cord).unwrap() {
            assert_eq!(value.sequence, sequence);
            assert_eq!(s.kernel().values().get(value.value).unwrap(), bytes);
            s.accept_owned_output(OUT.endpoint, OUT.cord, sequence)
                .unwrap();
            accepted = true;
            break;
        }
    }
    assert!(accepted);
    for _ in 0..8 {
        if s.step().unwrap() == SchedulerStatus::Drained {
            return;
        }
    }
    panic!("not normally drained")
}
#[test]
fn exact_owned_boundaries_preserve_sequences_resources_and_old_evidence() {
    let mut s = session();
    let address = s.kernel() as *const _ as usize;
    invoke(&mut s, 0, &[7]);
    let events = s.kernel().signs().events().collect::<alloc::vec::Vec<_>>();
    let old = s
        .begin_next([()], log(32))
        .unwrap_or_else(|_| panic!("drained"));
    assert_eq!(old.signs.events().collect::<alloc::vec::Vec<_>>(), events);
    assert_eq!(address, s.kernel() as *const _ as usize);
    assert_eq!(s.kernel().values().used_items(), 0);
    invoke(&mut s, 1, &[8, 9]);
    assert_eq!(s.kernel.cords[0].next_remote_sequence, 2);
    assert_eq!(s.kernel.cords[1].next_remote_sequence, 2);
}
#[test]
fn default_remote_refusal_and_exact_endpoint_admission_remain_distinct() {
    assert!(matches!(
        Session::new(kernel()),
        Err((SessionRefusal::UnsupportedRemote, _))
    ));
    for boundaries in [
        alloc::vec![IN],
        alloc::vec![IN, IN],
        alloc::vec![
            IN,
            OwnedValueBoundary {
                endpoint: RemoteEndpointId(9),
                ..OUT
            }
        ],
    ] {
        assert!(matches!(
            Session::new_with_owned_value_boundaries(kernel(), &boundaries, 16),
            Err((SessionRefusal::InvalidOwnedBoundary, _))
        ));
    }
    assert!(matches!(
        Session::new_with_owned_value_boundaries(kernel(), &[IN, OUT], 0),
        Err((SessionRefusal::EvidenceReservation, _))
    ));
}
#[test]
fn missing_remote_evidence_refuses_before_any_rearm_mutation() {
    let mut s = session();
    invoke(&mut s, 0, &[7]);
    let before = s.kernel().signs().events().collect::<alloc::vec::Vec<_>>();
    let refusal = s.begin_next([()], log(0)).err().unwrap();
    assert_eq!(refusal.reason, SessionRefusal::EvidenceReservation);
    assert_eq!(
        s.kernel().signs().events().collect::<alloc::vec::Vec<_>>(),
        before
    );
    assert_eq!(s.kernel.cords[0].next_remote_sequence, 1);
    s.begin_next([()], log(32))
        .unwrap_or_else(|_| panic!("untouched owner"));
    invoke(&mut s, 1, &[9]);
}
#[test]
fn pressure_stale_input_ack_failure_and_cancel_never_reprepare_or_replay() {
    let mut s = session();
    assert!(matches!(
        s.admit_owned_input(IN.endpoint, IN.cord, 0, &[7]),
        Ok(RemoteIngressOutcome::Accepted { .. })
    ));
    assert_eq!(
        s.admit_owned_input(IN.endpoint, IN.cord, 1, &[8]).unwrap(),
        RemoteIngressOutcome::Full { sequence: 1 }
    );
    assert!(s.begin_next([()], log(32)).is_err());
    s.close_owned_input(IN.endpoint, IN.cord).unwrap();
    s.step().unwrap();
    assert_eq!(s.kernel.cords[0].len, 0);
    assert_eq!(
        s.admit_owned_input(IN.endpoint, IN.cord, 1, &[8]).unwrap(),
        RemoteIngressOutcome::Full { sequence: 1 },
        "a consumed queue is still a live Value invocation until exact output acceptance"
    );
    let output = s.owned_output(OUT.endpoint, OUT.cord).unwrap().unwrap();
    assert!(s
        .accept_owned_output(OUT.endpoint, OUT.cord, output.sequence + 1)
        .is_err());
    assert!(s.begin_next([()], log(32)).is_err());
    s.cancel().unwrap();
    assert!(s.begin_next([()], log(32)).is_err());
    let mut s = session();
    invoke(&mut s, 0, &[7]);
    s.begin_next([()], log(32))
        .unwrap_or_else(|_| panic!("drained"));
    assert_eq!(
        s.admit_owned_input(IN.endpoint, IN.cord, 0, &[7]),
        Err(SchedulerError::RemoteSequenceRejected)
    );
    assert!(s.begin_next([()], log(32)).is_err());
}

#[test]
fn empty_value_closure_and_sequence_exhaustion_are_explicit_refusals() {
    let mut empty = session();
    assert_eq!(
        empty.close_owned_input(IN.endpoint, IN.cord),
        Err(SchedulerError::ValueOwnershipViolation)
    );
    assert!(empty.begin_next([()], log(32)).is_err());
    let mut s = session();
    invoke(&mut s, 0, &[7]);
    s.owned_value_index = u64::MAX - 1;
    for cord in &mut s.kernel.cords {
        cord.next_remote_sequence = u64::MAX;
    }
    let before = s.kernel().signs().events().collect::<alloc::vec::Vec<_>>();
    let refusal = s.begin_next([()], log(32)).err().unwrap();
    assert_eq!(refusal.reason, SessionRefusal::InvocationExhausted);
    assert_eq!(
        s.kernel().signs().events().collect::<alloc::vec::Vec<_>>(),
        before
    );
}
