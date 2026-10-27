use super::super::{NodeSpec, StepInputBytes, StepIo, StepOutcome};
use super::*;
use crate::{Failure, FailureCode, FixedRoutes, FixedSignLog, FixedValueStore, KernelEventKind};

struct Back {
    finished: bool,
    fail_once: bool,
    begins: usize,
    resident: Option<ValueRef>,
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
    fn preflight_next(&self, next: &bool, values: &dyn ValueStorage) -> bool {
        self.finished && *next && self.resident.is_none_or(|value| values.get(value).is_ok())
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
    let resident = store.store(&[1, 2, 3]).unwrap(); // retained live owner
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
                resident: Some(resident),
            },
            Back {
                finished: false,
                fail_once: false,
                begins: 0,
                resident: None,
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

#[test]
fn lost_resident_pin_refuses_before_reset_and_preserves_old_evidence() {
    let mut session = specimen(false);
    drain(&mut session);
    let resident = session.kernel().drivers()[0].resident.unwrap();
    assert_eq!(session.kernel().values().get(resident).unwrap(), &[1, 2, 3]);
    session.release_between_invocations(resident).unwrap();
    let count = session.kernel().signs().len();
    let refused = session.begin_next([true, true], log()).err().unwrap();
    assert_eq!(
        refused.reason,
        SessionRefusal::BackNotReady { node: NodeId(0) }
    );
    assert_eq!(session.kernel().drivers()[0].begins, 0);
    assert_eq!(session.kernel().signs().len(), count);
}

#[test]
fn normal_retirement_returns_exact_drivers_backing_and_evidence() {
    let mut session = specimen(false);
    drain(&mut session);
    let resident = session.kernel().drivers()[0].resident.unwrap();
    let events = session
        .kernel()
        .signs()
        .events()
        .collect::<alloc::vec::Vec<_>>();
    let decisions = session.kernel.decisions;
    let invocations = session.kernel.logical_invocations;
    assert!(session.retirement_ready());
    let retired = session
        .try_retire()
        .unwrap_or_else(|_| panic!("normal retirement"));
    assert_eq!(
        retired.signs.events().collect::<alloc::vec::Vec<_>>(),
        events
    );
    assert_eq!(retired.values.get(resident).unwrap(), &[1, 2, 3]);
    assert_eq!(retired.drivers[0].resident, Some(resident));
    assert!(retired.drivers.iter().all(|driver| driver.finished));
    assert_eq!(retired.decisions, decisions);
    assert_eq!(retired.logical_invocations, invocations);
    assert!(!retired.cancelled);
}

#[test]
fn retirement_refusal_returns_original_live_or_failed_session_unchanged() {
    let mut cases = alloc::vec![
        specimen(false),
        specimen(false),
        specimen(true),
        specimen(false)
    ];
    cases[1].step().unwrap(); // Only one of two nodes completed.
    assert!(cases[2].step().is_err());
    cases[3].cancel().unwrap();
    for session in cases {
        assert!(!session.retirement_ready());
        let address = session.kernel() as *const _ as usize;
        let events = session
            .kernel()
            .signs()
            .events()
            .collect::<alloc::vec::Vec<_>>();
        let used = session.kernel().values().used_bytes();
        let refused = session.try_retire().err().expect("not normally drained");
        assert_eq!(refused.kernel() as *const _ as usize, address);
        assert_eq!(
            refused
                .kernel()
                .signs()
                .events()
                .collect::<alloc::vec::Vec<_>>(),
            events
        );
        assert_eq!(refused.kernel().values().used_bytes(), used);
    }
}

#[test]
fn retirement_after_rearm_requires_new_normal_drain() {
    let mut session = specimen(false);
    drain(&mut session);
    session
        .begin_next([true, true], log())
        .unwrap_or_else(|_| panic!("rearm"));
    let mut session = session
        .try_retire()
        .err()
        .expect("fresh successor is active custody");
    drain(&mut session);
    let retired = session
        .try_retire()
        .unwrap_or_else(|_| panic!("fresh drain"));
    assert!(retired.drivers.iter().all(|driver| driver.begins == 1));
}
