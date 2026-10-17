use super::common::*;
use conduit_core::claims::*;
struct BadPolicy;
impl<D: ClaimDomain> ClaimPolicy<D> for BadPolicy {
    fn identity(&self) -> ClaimText<'_> {
        text("fixture/invalid-policy@1")
    }
    fn assess<'a>(
        &self,
        _: &SemanticClaim<'a, D>,
        _: &[&SemanticClaim<'a, D>],
    ) -> Option<ClaimReason<'a>> {
        Some(reason())
    }
    fn decide<'a>(
        &self,
        _: &[&SemanticClaim<'a, D>],
        _: &[bool],
        _: &[&SemanticClaim<'a, D>],
    ) -> ClaimDecision<'a> {
        ClaimDecision::Selected {
            identity: text("a"),
            reason: reason(),
        }
    }
}

#[test]
fn malformed_edges_cycles_identity_order_and_bounds_refuse() {
    assert_eq!(ClaimText::new(""), Err(ClaimRefusal::TextBound));
    assert_eq!(
        ClaimText::new(&"x".repeat(193)),
        Err(ClaimRefusal::TextBound)
    );
    let target = Object {
        track: 8,
        generation: 17,
    };
    let obs = observation();
    let domain = Vision;
    assert!(matches!(
        SemanticClaim::new(
            &domain,
            &target,
            &Classification::Person,
            basis("a", &[], &[])
        ),
        Err(ClaimRefusal::EdgeBound)
    ));
    let excessive = [obs[0]; 17];
    assert!(matches!(
        SemanticClaim::new(
            &domain,
            &target,
            &Classification::Person,
            basis("a", &excessive, &[])
        ),
        Err(ClaimRefusal::EdgeBound)
    ));
    let self_edge = [ClaimSupport::Claim(text("a"))];
    assert!(matches!(
        SemanticClaim::new(
            &domain,
            &target,
            &Classification::Person,
            basis("a", &self_edge, &[])
        ),
        Err(ClaimRefusal::InvalidEdge)
    ));
    let a_support = [ClaimSupport::Claim(text("b"))];
    let b_support = [ClaimSupport::Claim(text("a"))];
    let a = SemanticClaim::new(
        &domain,
        &target,
        &Classification::Person,
        basis("a", &a_support, &[]),
    )
    .unwrap();
    let b = SemanticClaim::new(
        &domain,
        &target,
        &Classification::Person,
        basis("b", &b_support, &[]),
    )
    .unwrap();
    assert!(matches!(
        resolve_claims(text("r"), &[&a, &b], &Conservative),
        Err(ClaimRefusal::CyclicSupport)
    ));
    assert!(matches!(
        resolve_claims(text("r"), &[&a], &Conservative),
        Err(ClaimRefusal::MissingSupport)
    ));
    assert!(matches!(
        resolve_claims(text("r"), &[&b, &a], &Conservative),
        Err(ClaimRefusal::NonCanonicalOrder)
    ));
    let plain = SemanticClaim::new(
        &domain,
        &target,
        &Classification::Person,
        basis("a", &obs, &[]),
    )
    .unwrap();
    assert!(matches!(
        resolve_claims(text("r"), &[&plain, &plain], &Conservative),
        Err(ClaimRefusal::DuplicateIdentity)
    ));
    assert!(matches!(
        resolve_claims(text("r"), &[&plain; 33], &Conservative),
        Err(ClaimRefusal::CandidateBound)
    ));
}

#[test]
fn separate_support_closure_preserves_other_targets_and_rejects_substitution() {
    let first_target = Object {
        track: 1,
        generation: 17,
    };
    let second_target = Object {
        track: 2,
        generation: 17,
    };
    let obs = observation();
    let support = [ClaimSupport::Claim(text("a"))];
    let domain = Vision;
    let a = SemanticClaim::new(
        &domain,
        &first_target,
        &Classification::Person,
        basis("a", &obs, &[]),
    )
    .unwrap();
    let b = SemanticClaim::new(
        &domain,
        &second_target,
        &Classification::Person,
        basis("b", &support, &[]),
    )
    .unwrap();
    let evidence = [&a, &b];
    let candidates = [&b];
    let result =
        resolve_claims_with_evidence(text("r"), &candidates, &evidence, &Conservative).unwrap();
    assert_eq!(result.evidence()[0].target().track, 1);
    let substitution = SemanticClaim::new(
        &domain,
        &second_target,
        &Classification::Mannequin,
        basis("b", &support, &[]),
    )
    .unwrap();
    assert!(matches!(
        resolve_claims_with_evidence(text("r"), &[&substitution], &evidence, &Conservative),
        Err(ClaimRefusal::MissingSupport)
    ));
    assert!(matches!(
        resolve_claims(text("r"), &evidence, &Conservative),
        Err(ClaimRefusal::TargetMismatch)
    ));
}

#[test]
fn policy_cannot_select_excluded_or_absent_claims() {
    let target = Object {
        track: 8,
        generation: 17,
    };
    let obs = observation();
    let domain = Vision;
    let a = SemanticClaim::new(
        &domain,
        &target,
        &Classification::Person,
        basis("a", &obs, &[]),
    )
    .unwrap();
    let b = SemanticClaim::new(
        &domain,
        &target,
        &Classification::Person,
        basis("b", &obs, &[]),
    )
    .unwrap();
    assert!(matches!(
        resolve_claims(text("r"), &[&a], &BadPolicy),
        Err(ClaimRefusal::InvalidDecision)
    ));
    assert!(matches!(
        resolve_claims(text("r"), &[&b], &BadPolicy),
        Err(ClaimRefusal::InvalidDecision)
    ));
}
