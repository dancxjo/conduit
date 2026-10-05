use super::common::*;
use conduit_core::claims::*;

#[test]
fn lifecycle_preserves_history_and_locks_committed_truth() {
    let target = Object {
        track: 8,
        generation: 17,
    };
    let obs = observation();
    let domain = Vision;
    let mut claim = SemanticClaim::new(
        &domain,
        &target,
        &Classification::Person,
        basis("a", &obs, &[]),
    )
    .unwrap();
    for state in [ClaimLifecycle::Stable, ClaimLifecycle::Committed] {
        claim
            .transition(ClaimChange {
                state,
                reason: reason(),
                replacement: None,
            })
            .unwrap();
    }
    assert_eq!(claim.history().len(), 2);
    assert_eq!(
        claim.transition(ClaimChange {
            state: ClaimLifecycle::Revised,
            reason: reason(),
            replacement: Some(text("b"))
        }),
        Err(ClaimRefusal::Lifecycle)
    );
    assert_eq!(claim.lifecycle(), ClaimLifecycle::Committed);
    assert_eq!(claim.value(), &Classification::Person);
    let correction_support = [ClaimSupport::Claim(text("a"))];
    let correction = SemanticClaim::new(
        &domain,
        &target,
        &Classification::Mannequin,
        basis("b", &correction_support, &[]),
    )
    .unwrap();
    assert_eq!(correction.value(), &Classification::Mannequin);
    let mut revised = SemanticClaim::new(
        &domain,
        &target,
        &Classification::Person,
        basis("c", &obs, &[]),
    )
    .unwrap();
    revised
        .transition(ClaimChange {
            state: ClaimLifecycle::Revised,
            reason: reason(),
            replacement: Some(text("b")),
        })
        .unwrap();
    let mut invalid = SemanticClaim::new(
        &domain,
        &target,
        &Classification::Person,
        basis("d", &obs, &[]),
    )
    .unwrap();
    invalid
        .transition(ClaimChange {
            state: ClaimLifecycle::Invalidated,
            reason: reason(),
            replacement: None,
        })
        .unwrap();
    let candidates = [&claim, &correction, &revised, &invalid];
    let resolution = resolve_claims(text("r2"), &candidates, &Conservative).unwrap();
    assert!(resolution.receipts()[2].unwrap().exclusion.is_some());
    assert!(resolution.receipts()[3].unwrap().exclusion.is_some());
    assert_eq!(
        resolution.candidates()[2].history()[0].unwrap().replacement,
        Some(text("b"))
    );
}

#[test]
fn confidence_requires_exact_score_contract_and_producer() {
    let score = |producer, calibration| {
        ClaimScore::new(
            text("probability/ppm@1"),
            calibration,
            text(producer),
            0,
            1_000_000,
            830_000,
        )
        .unwrap()
    };
    let a = score("model/a@1", None);
    assert_eq!(
        a.compare(&score("model/b@1", None)),
        Err(ClaimRefusal::UnrelatedScores)
    );
    assert_eq!(
        a.compare(&score("model/a@1", Some(text("calibration/2")))),
        Err(ClaimRefusal::UnrelatedScores)
    );
    assert_eq!(a.compare(&a), Ok(core::cmp::Ordering::Equal));
    assert_eq!(
        ClaimScore::new(text("scale"), None, text("producer"), 0, 10, 11),
        Err(ClaimRefusal::ScoreBound)
    );
}
