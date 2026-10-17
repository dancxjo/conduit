use super::common::*;
use conduit_core::projection::*;

#[test]
fn native_cost_is_not_confidence_and_normalization_needs_a_policy() {
    let domain = Bits;
    let policy = Budget(0);
    let source = 1;
    let facts = [ProjectionFact::Preserved {
        obligation: text("0"),
    }];
    let native = [ProjectionNativeFact {
        identity: text("cost"),
        contract: text("native-cost@1"),
        provider: text("provider@1"),
        encoding: text("integer@1"),
        bytes: b"17",
    }];
    let cost = score("cost/lower-is-better@1", "provider@1", 17);
    let confidence = score("confidence/percent@1", "provider@1", 17);
    assert!(cost.compare(&confidence).is_err());
    assert_eq!(
        cost.compare(&score("cost/lower-is-better@1", "provider@1", 18)),
        Ok(core::cmp::Ordering::Less)
    );
    let scores = [ProjectionScore::Native {
        fact: text("cost"),
        score: cost,
    }];
    let mut draft = input(&source, Some(&source), &facts);
    draft.native = &native;
    draft.scores = &scores;
    let report = ProjectionReport::new(&domain, &policy, draft).unwrap();
    assert_eq!(report.scores(), &scores);
    let normalized = [ProjectionScore::Normalized {
        fact: text("cost"),
        original: cost,
        projected: confidence,
        policy: text("wishful@1"),
    }];
    let mut draft = input(&source, Some(&source), &facts);
    draft.native = &native;
    draft.scores = &normalized;
    assert!(matches!(
        ProjectionReport::new(&domain, &policy, draft),
        Err(ProjectionRefusal::PolicyMismatch)
    ));
    let wrong = [ProjectionScore::Native {
        fact: text("cost"),
        score: score("cost@1", "different-provider", 17),
    }];
    let mut draft = input(&source, Some(&source), &facts);
    draft.native = &native;
    draft.scores = &wrong;
    assert!(matches!(
        ProjectionReport::new(&domain, &policy, draft),
        Err(ProjectionRefusal::Domain)
    ));
}

struct NormalizeCost;
impl ProjectionPolicy<Bits> for NormalizeCost {
    fn identity(&self) -> ProjectionText<'_> {
        text("strict-with-cost-inversion@1")
    }
    fn permits(&self, _: &u64, _: &u64, _: &[ProjectionFact<'_, Bits>]) -> bool {
        false
    }
    fn permits_normalization(
        &self,
        original: conduit_core::claims::ClaimScore<'_>,
        projected: conduit_core::claims::ClaimScore<'_>,
        policy: ProjectionText<'_>,
    ) -> bool {
        policy.as_str() == "invert-bounded-cost@1"
            && (0..=100).contains(&original.value())
            && original == score("integer-cost@1", "provider@1", original.value())
            && projected == score("inverted-cost@1", "provider@1", 100 - original.value())
    }
}
#[test]
fn declared_numerical_normalization_retains_original_and_refuses_wrong_mapping() {
    let domain = Bits;
    let policy = NormalizeCost;
    let source = 1;
    let facts = [ProjectionFact::Preserved {
        obligation: text("0"),
    }];
    let native = [ProjectionNativeFact {
        identity: text("cost"),
        contract: text("native-cost@1"),
        provider: text("provider@1"),
        encoding: text("integer@1"),
        bytes: b"17",
    }];
    let original = score("integer-cost@1", "provider@1", 17);
    let scores = [ProjectionScore::Normalized {
        fact: text("cost"),
        original,
        projected: score("inverted-cost@1", "provider@1", 83),
        policy: text("invert-bounded-cost@1"),
    }];
    let mut draft = input(&source, Some(&source), &facts);
    draft.native = &native;
    draft.scores = &scores;
    let report = ProjectionReport::new(&domain, &policy, draft).unwrap();
    assert_eq!(report.scores(), &scores);
    assert_eq!(report.require_exact(), Ok(&source));
    assert!(
        matches!(report.scores()[0],ProjectionScore::Normalized { original: retained,.. } if retained==original)
    );
    let wrong = [ProjectionScore::Normalized {
        fact: text("cost"),
        original,
        projected: score("inverted-cost@1", "provider@1", 17),
        policy: text("invert-bounded-cost@1"),
    }];
    let mut draft = input(&source, Some(&source), &facts);
    draft.native = &native;
    draft.scores = &wrong;
    assert!(matches!(
        ProjectionReport::new(&domain, &policy, draft),
        Err(ProjectionRefusal::PolicyMismatch)
    ));
}
