use super::*;

fn basis() -> ObligationBasis {
    ObligationBasis::current("commit/current".into())
}

#[test]
fn checked_planned_play_completes_the_exact_specimen() {
    let record = run_obligation(basis(), None, false, || true).unwrap();
    assert_eq!(record.terminal_verdict, Some(ObligationVerdict::Completed));
    assert!(record.checkpoint.is_none());
    assert_eq!(record.attempts.len(), 1);
    assert_eq!(record.attempts[0].verdict, ObligationVerdict::Completed);
    assert!(record.attempts[0].receipt.as_ref().unwrap().succeeded);
    assert!(!record.form_id.is_empty());
    assert!(!record.plan_id.is_empty());
    assert!(!record.attempts[0].play_id.is_empty());
    assert!(record.attempts[0]
        .signs
        .iter()
        .any(|sign| sign == "HostOperationRequested"));
    assert!(record.attempts[0]
        .signs
        .iter()
        .any(|sign| sign == "HostOperationCompleted"));
    assert_eq!(record.basis.command, SPECIMEN_COMMAND);
    assert_eq!(
        record.basis.proof_class,
        crate::proof::ProofClass::DeterministicUnit
    );
}

#[test]
fn interruption_yields_exact_residual_and_resume_starts_a_new_play() {
    let interrupted = run_obligation(basis(), None, true, || panic!("must not execute")).unwrap();
    assert_eq!(interrupted.terminal_verdict, None);
    assert!(interrupted.attempts[0].receipt.is_none());
    let checkpoint = interrupted.checkpoint.as_ref().unwrap();
    assert_eq!(checkpoint.residual, [ResidualStep::ExecuteProofCatalog]);
    let old_play = interrupted.attempts[0].play_id.clone();
    let resumed = run_obligation(basis(), Some(interrupted), false, || true).unwrap();
    assert_eq!(resumed.terminal_verdict, Some(ObligationVerdict::Completed));
    assert_eq!(resumed.attempts.len(), 2);
    assert_eq!(resumed.attempts[0].verdict, ObligationVerdict::Interrupted);
    assert_ne!(old_play, resumed.attempts[1].play_id);
}

#[test]
fn every_basis_and_checkpoint_mismatch_refuses_distinctly() {
    let interrupted = run_obligation(basis(), None, true, || true).unwrap();
    for (mut changed, expected) in [
        (
            {
                let mut value = basis();
                value.source_commit = "commit/stale".into();
                value
            },
            ObligationRefusal::StaleCommit,
        ),
        (
            {
                let mut value = basis();
                value.command = "cargo xtask check workspace".into();
                value
            },
            ObligationRefusal::ChangedCommand,
        ),
        (
            {
                let mut value = basis();
                value.profile = "profile/changed".into();
                value
            },
            ObligationRefusal::ChangedProfile,
        ),
        (
            {
                let mut value = basis();
                value.artifact_digest = "artifact/changed".into();
                value
            },
            ObligationRefusal::IncompatibleArtifact,
        ),
    ] {
        let mut prior = interrupted.clone();
        prior.checkpoint.as_mut().unwrap().basis = changed.clone();
        prior.basis = changed.clone();
        prior.obligation_id = obligation_id(&changed);
        assert_eq!(
            run_obligation(basis(), Some(prior), false, || true),
            Err(expected)
        );
        changed.source_commit.clear();
    }
    let mut corrupt = interrupted;
    corrupt.checkpoint.as_mut().unwrap().checkpoint_id = "checkpoint/corrupt".into();
    assert_eq!(
        run_obligation(basis(), Some(corrupt), false, || true),
        Err(ObligationRefusal::CorruptCheckpoint)
    );
}

#[test]
fn failed_attempt_remains_failure_and_attempt_budget_is_finite() {
    let failed = run_obligation(basis(), None, false, || false).unwrap();
    assert_eq!(failed.terminal_verdict, Some(ObligationVerdict::Failed));
    assert_eq!(failed.attempts[0].verdict, ObligationVerdict::Failed);
    assert!(!failed.attempts[0].receipt.as_ref().unwrap().succeeded);
    let recovered = run_obligation(basis(), Some(failed), false, || true).unwrap();
    assert_eq!(
        recovered.terminal_verdict,
        Some(ObligationVerdict::Completed)
    );
    assert_eq!(recovered.attempts[0].verdict, ObligationVerdict::Failed);
    assert_eq!(recovered.attempts[1].verdict, ObligationVerdict::Completed);

    let first = run_obligation(basis(), None, true, || true).unwrap();
    let second = run_obligation(basis(), Some(first), true, || true).unwrap();
    let third = run_obligation(basis(), Some(second), true, || true).unwrap();
    let fourth = run_obligation(basis(), Some(third), true, || true).unwrap();
    assert_eq!(fourth.retention_gap, 1);
    assert_eq!(fourth.attempts.len(), MAX_RETAINED_ATTEMPTS);
    assert_eq!(
        run_obligation(basis(), Some(fourth), false, || true),
        Err(ObligationRefusal::AttemptBudgetExhausted)
    );
}

#[test]
fn retention_gap_is_explicit_and_proof_class_cannot_be_promoted() {
    let record = run_obligation(basis(), None, false, || true).unwrap();
    assert_eq!(record.retention_gap, 0);
    assert!(record.attempts[0].signs.len() <= MAX_SIGNS);
    assert_ne!(
        record.basis.proof_class,
        crate::proof::ProofClass::LiveBrowser
    );
    assert_ne!(
        record.basis.proof_class,
        crate::proof::ProofClass::FirmwareBuild
    );
    assert_ne!(
        record.basis.proof_class,
        crate::proof::ProofClass::PhysicalCrossHost
    );
}
