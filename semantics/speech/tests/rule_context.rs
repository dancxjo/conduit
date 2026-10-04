#![cfg(feature = "semantic-bindings")]
use conduit_core::IeeeF32;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{
    declared_context::ExplicitAllophoneContext,
    occurrence_context::{OccurrenceContextRefusal, PlannedNeighbor},
    rule_conditions::ConditionRefusalReason,
    rule_context::*,
    semantic::*,
};
#[allow(dead_code)]
#[path = "common/occurrence_intent.rs"]
mod fixture;
fn environment(
    stress: StressSpecification,
    before: Vec<SpeechSegmentMatcher>,
    after: Vec<SpeechSegmentMatcher>,
) -> SpeechEnvironment {
    SpeechEnvironment::new(
        BoundedSequence::try_from_iter(after).unwrap(),
        BoundedSequence::try_from_iter(before).unwrap(),
        SpeechProsodicContextSpecification::unspecified(),
        stress,
        SpeechSyllablePositionSpecification::unspecified(),
        SpeechPositionSpecification::known(SpeechWordPosition::Medial).unwrap(),
    )
    .unwrap()
}
fn before() -> Vec<SpeechSegmentMatcher> {
    vec![SpeechSegmentMatcher::phone(PhoneId::new("phone/t".into()).unwrap()).unwrap()]
}
fn after() -> Vec<SpeechSegmentMatcher> {
    vec![SpeechSegmentMatcher::phoneme(PhonemeId::new("phoneme/t".into()).unwrap()).unwrap()]
}
fn rule(
    environment: SpeechEnvironment,
    conditions: Vec<SpeechRuleCondition>,
) -> SpeechAllophoneRule {
    SpeechAllophoneRule::new(
        BoundedSequence::try_from_iter(conditions).unwrap(),
        SpeechConfidence::new(IeeeF32::from_value(0.25)).unwrap(),
        environment,
        "original standalone rule".into(),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        PhoneSpecification::unknown(),
        PhonemeSpecification::unknown(),
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
    fn new(style: SpeechCarefulStyleSpecification) -> Self {
        Self {
            style,
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
#[test]
fn all_original_requirements_share_one_exact_occurrence_without_selecting_a_rule() {
    let intent = fixture::intent([
        fixture::segment(10),
        fixture::segment(11),
        fixture::segment(12),
    ]);
    let rule = rule(
        environment(
            StressSpecification::known(SpeechStress::Primary).unwrap(),
            before(),
            after(),
        ),
        vec![
            SpeechRuleCondition::not_careful_style(),
            SpeechRuleCondition::previous_stress(SpeechStress::Primary).unwrap(),
        ],
    );
    let explicit = Explicit::new(SpeechCarefulStyleSpecification::known(false).unwrap());
    let receipt = compare_allophone_rule_context(&rule, &intent, 1, explicit.get()).unwrap();
    assert_eq!(receipt.decision(), &SpeechContextDecision::Matched);
    assert!(core::ptr::eq(receipt.rule(), &rule));
    assert!(core::ptr::eq(receipt.occurrence().intent(), &intent));
    assert_eq!(receipt.occurrence().event(), 1);
    assert!(core::ptr::eq(
        receipt.stress().requirement(),
        rule.environment().stress_context()
    ));
    assert!(core::ptr::eq(
        receipt.stress().observation(),
        receipt.occurrence().segment().stress()
    ));
    assert!(core::ptr::eq(
        receipt.word_position().observation(),
        receipt.occurrence().segment().word_position()
    ));
    assert!(core::ptr::eq(
        receipt.syllable_position().observation(),
        &explicit.syllable
    ));
    assert!(core::ptr::eq(
        receipt.prosodic_context().observation(),
        &explicit.prosody
    ));
    assert!(core::ptr::eq(
        receipt.before().comparisons().next().unwrap().requirement(),
        &rule.environment().before().as_slice()[0]
    ));
    assert!(core::ptr::eq(
        receipt.after().comparisons().next().unwrap().requirement(),
        &rule.environment().after().as_slice()[0]
    ));
    assert!(core::ptr::eq(
        receipt.conditions().conditions(),
        rule.conditions()
    ));
    assert_eq!(receipt.conditions().receipts().count(), 2);
    assert!(matches!(
        receipt.rule().phone(),
        PhoneSpecification::Unknown
    ));
    assert!(matches!(
        receipt.rule().phoneme(),
        PhonemeSpecification::Unknown
    ));
    assert_eq!(receipt.rule().status(), &SpeechRuleStatus::Experimental);
}
#[test]
fn unknown_endpoints_and_real_boundaries_keep_distinct_neighbor_and_stress_views() {
    let intent = fixture::intent([fixture::segment(10), fixture::segment(11)]);
    let rule = rule(
        environment(StressSpecification::unspecified(), before(), after()),
        vec![],
    );
    let explicit = Explicit::new(SpeechCarefulStyleSpecification::known(false).unwrap());
    let receipt = compare_allophone_rule_context(&rule, &intent, 0, explicit.get()).unwrap();
    assert_eq!(
        receipt.before().decision(),
        &SpeechNeighborDecision::ObservationUnresolved
    );
    assert_eq!(
        receipt.decision(),
        &SpeechContextDecision::ObservationUnresolved
    );
    let intent = fixture::intent([
        fixture::segment(0),
        fixture::boundary(SpeechBoundarySpecification::known(SpeechBoundaryKind::Word).unwrap()),
        fixture::segment(1),
        fixture::segment(2),
    ]);
    let boundary_rule = make_boundary_rule();
    let receipt =
        compare_allophone_rule_context(&boundary_rule, &intent, 2, explicit.get()).unwrap();
    assert!(matches!(
        receipt.occurrence().before(),
        PlannedNeighbor::Boundary { event: 1, .. }
    ));
    assert_eq!(
        receipt.before().decision(),
        &SpeechNeighborDecision::Matched
    );
    assert_eq!(
        receipt.conditions().decision(),
        &SpeechContextDecision::Mismatched
    );
    assert_eq!(receipt.decision(), &SpeechContextDecision::Mismatched);
}
fn make_boundary_rule() -> SpeechAllophoneRule {
    rule(
        environment(
            StressSpecification::unspecified(),
            vec![SpeechSegmentMatcher::boundary(SpeechBoundaryKind::Word).unwrap()],
            after(),
        ),
        vec![SpeechRuleCondition::previous_stress(SpeechStress::Primary).unwrap()],
    )
}
#[test]
fn scalar_mismatch_and_requirement_uncertainty_retain_unresolved_components() {
    let intent = fixture::intent([
        fixture::segment(0),
        fixture::segment(1),
        fixture::segment(2),
    ]);
    let explicit = Explicit::new(SpeechCarefulStyleSpecification::unknown());
    for (stress, expected) in [
        (
            StressSpecification::known(SpeechStress::Unstressed).unwrap(),
            SpeechContextDecision::Mismatched,
        ),
        (
            StressSpecification::unknown(),
            SpeechContextDecision::RequirementUnresolved,
        ),
    ] {
        let rule = rule(
            environment(stress, before(), after()),
            vec![SpeechRuleCondition::not_careful_style()],
        );
        let receipt = compare_allophone_rule_context(&rule, &intent, 1, explicit.get()).unwrap();
        assert_eq!(receipt.decision(), &expected);
        assert_eq!(
            receipt.conditions().decision(),
            &SpeechContextDecision::ObservationUnresolved
        );
        assert_eq!(receipt.before().comparisons().count(), 1);
        assert_eq!(receipt.after().comparisons().count(), 1);
        assert_eq!(receipt.conditions().receipts().count(), 1);
    }
}
#[test]
fn occurrence_failures_remain_typed_before_any_context_comparison() {
    let rule = rule(
        environment(StressSpecification::unspecified(), before(), after()),
        vec![],
    );
    let explicit = Explicit::new(SpeechCarefulStyleSpecification::known(false).unwrap());
    let gap = fixture::intent([fixture::segment(10), fixture::segment(12)]);
    assert!(matches!(
        compare_allophone_rule_context(&rule, &gap, 0, explicit.get()),
        Err(RuleContextRefusal::Occurrence(
            OccurrenceContextRefusal::Neighbor { .. }
        ))
    ));
    let duplicate = fixture::intent([fixture::segment(0), fixture::segment(0)]);
    assert!(matches!(
        compare_allophone_rule_context(&rule, &duplicate, 0, explicit.get()),
        Err(RuleContextRefusal::Occurrence(
            OccurrenceContextRefusal::DuplicateOccurrence { .. }
        ))
    ));
    let boundary = fixture::intent([fixture::boundary(SpeechBoundarySpecification::unknown())]);
    assert!(matches!(
        compare_allophone_rule_context(&rule, &boundary, 0, explicit.get()),
        Err(RuleContextRefusal::Occurrence(
            OccurrenceContextRefusal::BoundaryEvent
        ))
    ));
    assert!(matches!(
        compare_allophone_rule_context(&rule, &boundary, usize::MAX, explicit.get()),
        Err(RuleContextRefusal::Occurrence(
            OccurrenceContextRefusal::MissingEvent
        ))
    ));
}
#[test]
fn unsupported_syntax_is_an_indexed_refusal_even_after_a_scalar_mismatch() {
    let intent = fixture::intent([
        fixture::segment(0),
        fixture::segment(1),
        fixture::segment(2),
    ]);
    let explicit = Explicit::new(SpeechCarefulStyleSpecification::known(false).unwrap());
    let rule = rule(
        environment(
            StressSpecification::known(SpeechStress::Unstressed).unwrap(),
            before(),
            after(),
        ),
        vec![
            SpeechRuleCondition::not_careful_style(),
            SpeechRuleCondition::current_word_has_syntactic_link(SpeechSyntacticLinkKind::Subject)
                .unwrap(),
        ],
    );
    let Err(RuleContextRefusal::Conditions(refusal)) =
        compare_allophone_rule_context(&rule, &intent, 1, explicit.get())
    else {
        panic!()
    };
    assert_eq!(refusal.index, 1);
    assert!(matches!(
        refusal.reason,
        ConditionRefusalReason::UnsupportedSyntax
    ));
}
#[test]
fn full_alternative_and_condition_bounds_are_retained_on_the_same_neighbors() {
    let intent = fixture::intent([
        fixture::segment(0),
        fixture::segment(1),
        fixture::segment(2),
    ]);
    let explicit = Explicit::new(SpeechCarefulStyleSpecification::known(false).unwrap());
    let alternatives = || {
        (0..4)
            .map(|index| {
                SpeechSegmentMatcher::phone(
                    PhoneId::new(if index == 3 {
                        "phone/t".into()
                    } else {
                        format!("wrong/{index}")
                    })
                    .unwrap(),
                )
                .unwrap()
            })
            .collect::<Vec<_>>()
    };
    let rule = rule(
        environment(
            StressSpecification::unspecified(),
            alternatives(),
            alternatives(),
        ),
        vec![SpeechRuleCondition::not_careful_style(); 8],
    );
    let receipt = compare_allophone_rule_context(&rule, &intent, 1, explicit.get()).unwrap();
    assert_eq!(receipt.decision(), &SpeechContextDecision::Matched);
    assert_eq!(receipt.before().comparisons().count(), 4);
    assert_eq!(receipt.after().comparisons().count(), 4);
    assert_eq!(receipt.conditions().receipts().count(), 8);
}
