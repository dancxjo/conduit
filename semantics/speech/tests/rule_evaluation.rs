#![cfg(feature = "semantic-bindings")]
use conduit_core::IeeeF32;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{declared_context::ExplicitAllophoneContext, rule_evaluation::*, semantic::*};
#[allow(dead_code)]
#[path = "common/occurrence_intent.rs"]
mod fixture;
fn features(value: bool) -> SpeechFeatureBundle {
    SpeechFeatureBundle::new(
        BoundedSequence::try_from_iter([SpeechFeature::new(
            SpeechFeatureId::new("voiced".into()).unwrap(),
            FeatureSpecification::known(SpeechFeatureValue::boolean(value).unwrap()).unwrap(),
        )
        .unwrap()])
        .unwrap(),
    )
    .unwrap()
}
fn rule(id: &str, stress: StressSpecification) -> SpeechAllophoneRule {
    SpeechAllophoneRule::new(
        BoundedSequence::new(),
        SpeechConfidence::new(IeeeF32::from_value(0.25)).unwrap(),
        SpeechEnvironment::new(
            BoundedSequence::new(),
            BoundedSequence::new(),
            SpeechProsodicContextSpecification::unspecified(),
            stress,
            SpeechSyllablePositionSpecification::unspecified(),
            SpeechPositionSpecification::unspecified(),
        )
        .unwrap(),
        "original combined rule".into(),
        features(true),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        PhoneSpecification::unknown(),
        PhonemeSpecification::known(PhonemeId::new(id.into()).unwrap()).unwrap(),
        SpeechRuleStatus::Experimental,
    )
    .unwrap()
}
struct Explicit {
    style: SpeechCarefulStyleSpecification,
    syllable: SpeechSyllablePositionSpecification,
    prosody: SpeechProsodicContextSpecification,
}
impl Explicit {
    fn new() -> Self {
        Self {
            style: SpeechCarefulStyleSpecification::known(false).unwrap(),
            syllable: SpeechSyllablePositionSpecification::unknown(),
            prosody: SpeechProsodicContextSpecification::unknown(),
        }
    }
    fn get(&self) -> ExplicitAllophoneContext<'_> {
        ExplicitAllophoneContext {
            careful_style: &self.style,
            syllable_position: &self.syllable,
            prosodic_context: &self.prosody,
        }
    }
}
fn observation(
    occurrence: LanguageSpeechTokenRef,
    value: bool,
) -> SpeechOccurrenceFeatureObservation {
    SpeechOccurrenceFeatureObservation::new(
        features(value),
        occurrence,
        SpeechEvidenceProvenance::new(
            "explicit current feature fixture".into(),
            SpeechEvidenceSource::Manual,
            None,
        )
        .unwrap(),
    )
    .unwrap()
}
#[test]
fn input_and_context_share_exact_original_occurrence_and_feature_provenance() {
    let intent = fixture::intent([fixture::segment(10)]);
    let rule = rule(
        "phoneme/t",
        StressSpecification::known(SpeechStress::Primary).unwrap(),
    );
    let explicit = Explicit::new();
    let observed = observation(fixture::token(10, fixture::BASIS), true);
    let receipt =
        evaluate_allophone_rule(&rule, &intent, 0, Some(&observed), explicit.get()).unwrap();
    assert_eq!(receipt.decision(), &SpeechContextDecision::Matched);
    assert!(core::ptr::eq(receipt.rule(), &rule));
    assert!(core::ptr::eq(
        receipt.input().rule(),
        receipt.context().rule()
    ));
    assert!(core::ptr::eq(
        receipt.context().occurrence().intent(),
        &intent
    ));
    assert!(core::ptr::eq(
        receipt.input().phoneme().observation(),
        receipt.context().occurrence().segment().phoneme()
    ));
    assert!(core::ptr::eq(
        receipt.observed_features().unwrap(),
        &observed
    ));
    assert!(core::ptr::eq(
        receipt.input().features().observations().unwrap(),
        observed.features()
    ));
    assert_eq!(
        receipt.checked_features().unwrap().expected(),
        receipt.context().occurrence().segment().occurrence()
    );
    assert_eq!(
        receipt.checked_features().unwrap().observed(),
        observed.occurrence()
    );
    assert!(matches!(
        receipt.rule().phone(),
        PhoneSpecification::Unknown
    ));
    assert_eq!(receipt.rule().status(), &SpeechRuleStatus::Experimental);
}
#[test]
fn every_foreign_feature_occurrence_axis_refuses_without_becoming_mismatch() {
    let intent = fixture::intent([fixture::segment(10)]);
    let rule = rule("phoneme/t", StressSpecification::unspecified());
    let explicit = Explicit::new();
    for axis in 0..5 {
        let mut basis = fixture::BASIS;
        basis[axis] = "foreign";
        let observed = observation(fixture::token(10, basis), true);
        assert!(matches!(
            evaluate_allophone_rule(&rule, &intent, 0, Some(&observed), explicit.get()),
            Err(RuleEvaluationRefusal::FeatureOccurrence(_))
        ));
    }
    let wrong_ordinal = observation(fixture::token(11, fixture::BASIS), true);
    assert!(matches!(
        evaluate_allophone_rule(&rule, &intent, 0, Some(&wrong_ordinal), explicit.get()),
        Err(RuleEvaluationRefusal::FeatureOccurrence(_))
    ));
}
#[test]
fn absent_features_preserve_input_uncertainty_and_context_mismatch() {
    let intent = fixture::intent([fixture::segment(0)]);
    let explicit = Explicit::new();
    for (stress, expected) in [
        (
            StressSpecification::unspecified(),
            SpeechContextDecision::ObservationUnresolved,
        ),
        (
            StressSpecification::known(SpeechStress::Unstressed).unwrap(),
            SpeechContextDecision::Mismatched,
        ),
        (
            StressSpecification::unknown(),
            SpeechContextDecision::RequirementUnresolved,
        ),
    ] {
        let rule = rule("phoneme/t", stress);
        let receipt = evaluate_allophone_rule(&rule, &intent, 0, None, explicit.get()).unwrap();
        assert_eq!(receipt.decision(), &expected);
        assert_eq!(
            receipt.input().decision(),
            &SpeechContextDecision::ObservationUnresolved
        );
        assert!(receipt.observed_features().is_none());
        assert!(receipt.checked_features().is_none());
    }
}
#[test]
fn input_mismatch_keeps_unresolved_context_and_present_feature_evidence() {
    let intent = fixture::intent([fixture::segment(0)]);
    let explicit = Explicit::new();
    let rule = rule("other/phoneme", StressSpecification::unknown());
    let observed = observation(fixture::token(0, fixture::BASIS), false);
    let receipt =
        evaluate_allophone_rule(&rule, &intent, 0, Some(&observed), explicit.get()).unwrap();
    assert_eq!(receipt.decision(), &SpeechContextDecision::Mismatched);
    assert_eq!(
        receipt.context().decision(),
        &SpeechContextDecision::RequirementUnresolved
    );
    assert_eq!(
        receipt.input().features().decision(),
        &SpeechContextDecision::Mismatched
    );
    assert!(core::ptr::eq(
        receipt.observed_features().unwrap(),
        &observed
    ));
}
