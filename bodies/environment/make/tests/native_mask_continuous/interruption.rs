//! Actual clean finite-prefix settlement, not a normal completion receipt.
use super::*;
#[test]
fn cancelled_prefix_returns_distinct_mask_owner_before_exact_body_lull() {
    let _serial = WORKER_PROOF.lock().unwrap();
    CLOCK.store(1000, Ordering::Relaxed);
    let mut current = fixture(false);
    assert_eq!(run(&mut current), Ok(3));
    assert!(current.session.session.retirement_ready());
    assert!(current
        .session
        .retirement_ready(&current.leases.0, &current.evidence.consumer)
        .is_err());
    // A scheduling limit alone is not actual revocation authority.
    assert!(current
        .session
        .interruption_ready(&current.leases.0, &current.evidence.consumer)
        .is_err());
    current._guard.revoke();
    current
        .session
        .interruption_ready(&current.leases.0, &current.evidence.consumer)
        .unwrap();
    let old_play = current._body.realization().unwrap().play.clone().unwrap();
    let retired = current
        .session
        .retire_interrupted(
            Box::new(current.leases.0),
            Box::new(current.evidence.consumer),
        )
        .unwrap_or_else(|_| panic!("actual withdrawn/evidenced prefix refused"));
    assert_eq!(retired.settled_children(), 3);
    let mut wrong = current.initial.clone();
    wrong.play_sequence += 1;
    assert_eq!(
        current
            .root
            .finish_interrupted_mask_play(&current._body, &wrong, &retired),
        Err("mask-interruption-issuer-scope")
    );
    current
        .root
        .finish_interrupted_mask_play(&current._body, &current.initial, &retired)
        .unwrap();
    assert!(current
        .root
        .finish_interrupted_mask_play(&current._body, &current.initial, &retired)
        .is_err());
    // The actual lifecycle accepts the exact current BodyPlay after the Host
    // has returned the distinct settled interruption owner. No MAX count is
    // synthesized and no cancelled child kernel is relabelled as completed.
    current
        ._body
        .lull(&current.root.host, &current.root.boot, Some(&old_play))
        .unwrap();
    assert_eq!(
        current._body.evidence().body.state,
        conduit_body::BodyState::Lulled
    );
    let parts = retired.into_parts();
    assert!(!parts.kernel.cancelled);
    assert_eq!(
        parts.evidence.completion(),
        (3, Some(current.initial.play_sequence + 2))
    );
}

struct MissedWindow<'a>(&'a mut Limited);
impl worker::LeaseSource for MissedWindow<'_> {
    fn current(&self, now: u64) -> bool {
        self.0.current(now)
    }
    fn next(
        &mut self,
        owner: &mask_session::NativeReady,
        now: u64,
    ) -> Result<Option<worker::Lease>, &'static str> {
        self.0.next(owner, now)
    }
    fn stage_successor(
        &mut self,
        native: &RendererIdentity,
        previous: &c::ActivePlayIdentity,
        _now: u64,
    ) -> Result<Option<worker::Lease>, &'static str> {
        // An actual deterministic clock jump misses the complete fixed window.
        let now = CLOCK.fetch_add(3000, Ordering::Relaxed) + 3000;
        self.0.stage_successor(native, previous, now)
    }
    fn replaced(
        &mut self,
        native: &RendererIdentity,
        previous: &c::ActivePlayIdentity,
        old: &c::CapabilityInspection,
        current: &c::CapabilityInspection,
    ) -> Result<(), &'static str> {
        self.0.replaced(native, previous, old, current)
    }
    fn retired(
        &mut self,
        owner: &mask_session::NativeReady,
        active: &c::ActivePlayIdentity,
        inspection: &c::CapabilityInspection,
    ) -> Result<(), &'static str> {
        self.0.retired(owner, active, inspection)
    }
}
#[test]
fn missed_child_window_retains_actual_occurrence_and_forbids_retirement() {
    let _serial = WORKER_PROOF.lock().unwrap();
    CLOCK.store(1000, Ordering::Relaxed);
    let mut current = fixture(false);
    let result = {
        let mut leases = MissedWindow(&mut current.leases);
        let mut future = std::pin::pin!(current.session.run(&mut leases, &mut current.evidence));
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        match std::future::Future::poll(future.as_mut(), &mut context) {
            std::task::Poll::Ready(result) => result,
            std::task::Poll::Pending => panic!("deterministic clock unexpectedly pending"),
        }
    };
    assert_eq!(result, Err("mask-child-window"));
    assert!(current.session.faulted);
    assert_eq!(
        current.session.call.as_ref().unwrap().state(),
        RendererState::Withdrawn
    );
    assert!(current.session.cleanup_refusal().is_none());
    let retained = current
        .session
        .handoff
        .as_ref()
        .expect("failure must pin actual witness");
    assert!(matches!(
        retained.presentation.as_ref(),
        Some(mask_session::RetainedPresentation::Presented(_))
    ));
    assert!(retained.previous.is_none());
    assert!(retained.receipt.is_none());
    assert!(current.evidence.kinds.is_empty());
    assert!(std::sync::Arc::strong_count(&current.session.context) >= 2);
    current._guard.revoke();
    assert!(current
        .session
        .retirement_ready(&current.leases.0, &current.evidence.consumer)
        .is_err());
    assert!(current
        .session
        .interruption_ready(&current.leases.0, &current.evidence.consumer)
        .is_err());
    #[allow(dead_code)]
    struct PriorHandoff {
        previous_log: Option<mask_session::Log>,
        receipt: Option<ReplacedRenderer>,
        successor: Option<ReplacementLease>,
        refused: Option<(
            c::ActivePlayIdentity,
            c::BaseCapabilityTable,
            c::BaseCapabilityHandle,
            c::BaseOperationClaim,
        )>,
    }
    eprintln!(
        "prior_handoff_host_bytes={}",
        core::mem::size_of::<PriorHandoff>()
    );
    eprintln!("retained_handoff_host_bytes={} retained_state_host_bytes={} prepared_host_bytes={} presented_host_bytes={}",
        core::mem::size_of::<mask_session::Handoff>(),core::mem::size_of::<mask_session::RetainedPresentation>(),
        core::mem::size_of::<PreparedPresentation<'static>>(),core::mem::size_of::<PresentedOccurrence<'static>>());
}
