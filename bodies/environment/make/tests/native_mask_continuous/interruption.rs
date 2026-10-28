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
