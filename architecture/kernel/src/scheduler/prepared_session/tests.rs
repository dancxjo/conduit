use super::super::{NodeSpec, StepInputBytes, StepIo, StepOutcome};
use super::*;
use crate::{Failure, FailureCode, FixedRoutes, FixedSignLog, FixedValueStore, KernelEventKind};

struct Back {
    finished: bool,
    fail_once: bool,
    begins: usize,
}
impl StepBack<1> for Back {
    fn step(&mut self, _: &mut StepIo<1>, _: &StepInputBytes<'_, 1>) -> StepOutcome {
        if self.fail_once {
            self.fail_once = false;
            return StepOutcome::Fail(Failure {
                code: FailureCode::InvalidInput,
                detail: 17,
            });
        }
        StepOutcome::Complete
    }
    fn step_committed(&mut self) {
        self.finished = true;
    }
}
impl ReusableStepBack<1> for Back {
    type Next = bool;
    fn preflight_next(&self, next: &bool, _: &dyn ValueStorage) -> bool {
        self.finished && *next
    }
    fn begin_next(&mut self, _: bool) {
        self.finished = false;
        self.begins += 1;
    }
}
type Log = FixedSignLog<32>;
type Session = PreparedKernelSession<Back, FixedValueStore<4, 16>, Log, 2, 0, 1, 1, 2, 0>;
fn log() -> Log {
    Log::new(32 * core::mem::size_of::<crate::KernelEvent>() as u32).unwrap()
}
fn specimen(fail_once: bool) -> Session {
    let mut routes = FixedRoutes::<2, 0>::new(1);
    routes.seal().unwrap();
    let mut store = FixedValueStore::<4, 16>::new(64).unwrap();
    store.store(&[1, 2, 3]).unwrap(); // resident owner, no implicit store clearing
    let kernel = FixedScheduler::new_with_active_counts(
        2,
        0,
        [NodeSpec {
            input_cords: [None],
            maximum_step_fuel: 8,
        }; 2],
        [],
        routes,
        [
            Back {
                finished: false,
                fail_once,
                begins: 0,
            },
            Back {
                finished: false,
                fail_once: false,
                begins: 0,
            },
        ],
        store,
        log(),
    )
    .unwrap();
    Session::new(Box::new(kernel)).unwrap_or_else(|_| panic!("fresh"))
}
fn drain(session: &mut Session) {
    for _ in 0..8 {
        if session.step().unwrap() == SchedulerStatus::Drained {
            return;
        }
    }
    panic!("not drained")
}
#[test]
fn same_kernel_resources_and_exact_old_evidence_survive_successful_rearm() {
    let mut session = specimen(false);
    let address = session.kernel() as *const _ as usize;
    let used = session.kernel().values().used_bytes();
    drain(&mut session);
    let events: alloc::vec::Vec<_> = session.kernel().signs().events().collect();
    let old = session
        .begin_next([true, true], log())
        .unwrap_or_else(|_| panic!("normal rearm"));
    assert_eq!(old.signs.events().collect::<alloc::vec::Vec<_>>(), events);
    assert_eq!(old.logical_invocations, 2);
    assert_eq!(session.kernel().values().used_bytes(), used);
    assert_eq!(session.kernel() as *const _ as usize, address);
    assert_eq!(session.kernel().drivers()[0].begins, 1);
    assert!(session.kernel().signs().is_empty());
    drain(&mut session);
    assert_eq!(
        session
            .kernel()
            .signs()
            .events()
            .collect::<alloc::vec::Vec<_>>(),
        events
    );
    assert_eq!(
        session.kernel().logical_invocations(),
        old.logical_invocations
    );
}
#[test]
fn all_back_preflights_precede_any_reset_or_evidence_transfer() {
    let mut session = specimen(false);
    drain(&mut session);
    let count = session.kernel().signs().len();
    let error = session.begin_next([true, false], log()).err().unwrap();
    assert_eq!(
        error.reason,
        SessionRefusal::BackNotReady { node: NodeId(1) }
    );
    assert_eq!(error.next, [true, false]);
    assert!(error.signs.is_empty());
    assert_eq!(session.kernel().signs().len(), count);
    assert_eq!(session.kernel().drivers()[0].begins, 0);
    session
        .begin_next([true, true], error.signs)
        .unwrap_or_else(|_| panic!("retry admission"));
}
#[test]
fn earlier_failure_cannot_be_laundered_by_a_later_structural_drain() {
    let mut session = specimen(true);
    assert!(session.step().is_err());
    drain(&mut session);
    let error = session.begin_next([true, true], log()).err().unwrap();
    assert_eq!(error.reason, SessionRefusal::NotNormallyDrained);
    assert!(session
        .kernel()
        .signs()
        .contains_kind(KernelEventKind::BackFailed));
    assert_eq!(session.kernel().drivers()[0].begins, 0);
}
#[test]
fn active_cancelled_and_nonempty_evidence_reservations_refuse() {
    let mut session = specimen(false);
    assert_eq!(
        session
            .begin_next([true, true], log())
            .err()
            .unwrap()
            .reason,
        SessionRefusal::NotNormallyDrained
    );
    drain(&mut session);
    let mut occupied = log();
    occupied
        .record(NodeId(0), None, None, KernelEventKind::BackCompleted)
        .unwrap();
    let refused = session.begin_next([true, true], occupied).err().unwrap();
    assert_eq!(refused.reason, SessionRefusal::EvidenceReservation);
    assert_eq!(refused.signs.len(), 1);
    session.cancel().unwrap();
    assert_eq!(
        session
            .begin_next([true, true], log())
            .err()
            .unwrap()
            .reason,
        SessionRefusal::NotNormallyDrained
    );
}
