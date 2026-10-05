use super::common::*;
use conduit_core::claims::*;

#[test]
fn sensor_rule_model_and_manual_correction_retain_losers_and_exact_evidence() {
    let target = Object {
        track: 8,
        generation: 17,
    };
    let obs = observation();
    let rule_support = [ClaimSupport::Claim(text("c0/sensor"))];
    let conflicts = [text("c2/model")];
    let domain = Vision;
    let sensor = SemanticClaim::new(
        &domain,
        &target,
        &Classification::Person,
        basis("c0/sensor", &obs, &[]),
    )
    .unwrap();
    let rule = SemanticClaim::new(
        &domain,
        &target,
        &Classification::Person,
        basis("c1/rule", &rule_support, &conflicts),
    )
    .unwrap();
    let model_conflicts = [text("c1/rule")];
    let mut model_basis = basis("c2/model", &obs, &model_conflicts);
    model_basis.producer = text("vision/model-back@2");
    model_basis.artifact = text("vision/model-digest/5");
    model_basis.score = Some(
        ClaimScore::new(
            text("classification/logit@1"),
            None,
            model_basis.producer,
            -1000,
            1000,
            830,
        )
        .unwrap(),
    );
    let model =
        SemanticClaim::new(&domain, &target, &Classification::Mannequin, model_basis).unwrap();
    let manual_obs = [ClaimSupport::SourceGeneration {
        source: text("human/annotation"),
        generation: text("annotation/4"),
    }];
    let manual = SemanticClaim::new(
        &domain,
        &target,
        &Classification::Person,
        basis("c3/manual", &manual_obs, &[]),
    )
    .unwrap();
    let candidates = [&sensor, &rule, &model, &manual];
    let policy = ManualPolicy;
    let first = resolve_claims(text("r1"), &candidates, &policy).unwrap();
    let again = resolve_claims(text("r1"), &candidates, &policy).unwrap();
    assert_eq!(first.decision(), again.decision());
    assert_eq!(first.receipts(), again.receipts());
    assert!(
        matches!(first.decision(), ClaimDecision::Selected { identity, .. } if identity == text("c3/manual"))
    );
    assert_eq!(first.candidates()[2].value(), &Classification::Mannequin);
    assert_eq!(first.candidates()[1].basis().support, &rule_support);
    // A resolution contains semantic evidence, never an AuthorityGrant or Host call.
    assert_eq!(first.candidates().len(), 4);
}
