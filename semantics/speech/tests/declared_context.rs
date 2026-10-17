#![cfg(feature = "semantic-bindings")]
use conduit_core::IeeeF32;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{
    declared_context::*, declared_realization::*, occurrence_context::PlannedNeighbor,
    rule_conditions::ConditionRefusalReason, semantic::*,
};
#[allow(dead_code)]
#[path = "common/occurrence_intent.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "common/native_phone.rs"]
mod phone;
fn inventory(
    stress: SpeechStress,
    conditions: impl IntoIterator<Item = SpeechRuleCondition>,
) -> SpeechInventory {
    let environment = SpeechEnvironment::new(
        BoundedSequence::try_from_iter(
            [SpeechSegmentMatcher::phone(phone::id("phone/t")).unwrap()],
        )
        .unwrap(),
        BoundedSequence::try_from_iter([SpeechSegmentMatcher::phoneme(
            PhonemeId::new("phoneme/t".into()).unwrap(),
        )
        .unwrap()])
        .unwrap(),
        SpeechProsodicContextSpecification::unspecified(),
        StressSpecification::known(stress).unwrap(),
        SpeechSyllablePositionSpecification::unspecified(),
        SpeechPositionSpecification::known(SpeechWordPosition::Medial).unwrap(),
    )
    .unwrap();
    let declaration = SpeechPhonemeAllophone::new(
        BoundedSequence::try_from_iter(conditions).unwrap(),
        SpeechConfidence::new(IeeeF32::from_value(0.25)).unwrap(),
        environment,
        phone::id("phone/t"),
        Some("original rule".into()),
        SpeechRuleStatus::Experimental,
    )
    .unwrap();
    let phoneme = SpeechPhoneme::new(
        BoundedSequence::new(),
        BoundedSequence::try_from_iter([declaration]).unwrap(),
        Some(phone::id("phone/t")),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        PhonemeId::new("phoneme/t".into()).unwrap(),
        "t".into(),
        BoundedSequence::new(),
        SpeechSegmentStatus::Core,
    )
    .unwrap();
    SpeechInventory::new(
        SpeechInventoryId::new("inventory".into()).unwrap(),
        LanguageId::new("en".into()).unwrap(),
        BoundedSequence::try_from_iter([phoneme]).unwrap(),
        BoundedSequence::try_from_iter([phone::definition("phone/t")]).unwrap(),
    )
    .unwrap()
}
fn explicit<'a>(
    style: &'a SpeechCarefulStyleSpecification,
    syllable: &'a SpeechSyllablePositionSpecification,
    prosody: &'a SpeechProsodicContextSpecification,
) -> ExplicitAllophoneContext<'a> {
    ExplicitAllophoneContext {
        careful_style: style,
        syllable_position: syllable,
        prosodic_context: prosody,
    }
}
#[test]
fn original_declaration_gets_one_consistent_occurrence_context_without_becoming_selected() {
    let intent = fixture::intent([
        fixture::segment(10),
        fixture::segment(11),
        fixture::segment(12),
    ]);
    let inventory = inventory(
        SpeechStress::Primary,
        [
            SpeechRuleCondition::not_careful_style(),
            SpeechRuleCondition::previous_stress(SpeechStress::Primary).unwrap(),
        ],
    );
    let declared = resolve_declared_intent_realization(&intent, 1, &inventory).unwrap();
    let style = SpeechCarefulStyleSpecification::known(false).unwrap();
    let syllable = SpeechSyllablePositionSpecification::unknown();
    let prosody = SpeechProsodicContextSpecification::unknown();
    let context =
        compare_declared_allophone_context(&declared, 1, explicit(&style, &syllable, &prosody))
            .unwrap();
    assert_eq!(context.decision(), &SpeechContextDecision::Matched);
    assert!(core::ptr::eq(context.declared(), &declared));
    assert!(core::ptr::eq(context.occurrence().intent(), &intent));
    assert_eq!(context.occurrence().event(), 1);
    let original = &inventory.phonemes().as_slice()[0].allophones().as_slice()[0];
    assert!(core::ptr::eq(context.scalar().declaration(), original));
    assert!(core::ptr::eq(context.neighbors().declaration(), original));
    assert!(core::ptr::eq(
        context.conditions().conditions(),
        original.conditions()
    ));
    assert_eq!(
        context.scalar().declaration().status(),
        &SpeechRuleStatus::Experimental
    );
    assert_eq!(
        context.scalar().declaration().confidence(),
        original.confidence()
    );
    assert!(core::ptr::eq(
        context.scalar().stress().observation(),
        context.occurrence().segment().stress()
    ));
    assert!(matches!(
        context.occurrence().before(),
        PlannedNeighbor::Segment { event: 0, .. }
    ));
    assert_eq!(context.conditions().receipts().count(), 2);
    assert!(matches!(
        compare_declared_allophone_context(&declared, 0, explicit(&style, &syllable, &prosody)),
        Err(DeclaredContextRefusal::NotAllophone)
    ));
    assert!(matches!(
        compare_declared_allophone_context(
            &declared,
            usize::MAX,
            explicit(&style, &syllable, &prosody)
        ),
        Err(DeclaredContextRefusal::MissingDeclaration)
    ));
}
#[test]
fn scalar_mismatch_keeps_unresolved_conditions_and_every_original_component() {
    let intent = fixture::intent([
        fixture::segment(10),
        fixture::segment(11),
        fixture::segment(12),
    ]);
    let inventory = inventory(
        SpeechStress::Secondary,
        [SpeechRuleCondition::not_careful_style()],
    );
    let declared = resolve_declared_intent_realization(&intent, 1, &inventory).unwrap();
    let style = SpeechCarefulStyleSpecification::unknown();
    let syllable = SpeechSyllablePositionSpecification::unknown();
    let prosody = SpeechProsodicContextSpecification::unknown();
    let context =
        compare_declared_allophone_context(&declared, 1, explicit(&style, &syllable, &prosody))
            .unwrap();
    assert_eq!(context.decision(), &SpeechContextDecision::Mismatched);
    assert_eq!(
        context.scalar().stress().decision(),
        &SpeechContextDecision::Mismatched
    );
    assert_eq!(
        context.conditions().decision(),
        &SpeechContextDecision::ObservationUnresolved
    );
    assert_eq!(context.conditions().receipts().count(), 1);
}
#[test]
fn unobserved_endpoint_cannot_satisfy_a_required_neighbor() {
    let intent = fixture::intent([fixture::segment(10), fixture::segment(11)]);
    let inventory = inventory(SpeechStress::Primary, []);
    let declared = resolve_declared_intent_realization(&intent, 0, &inventory).unwrap();
    let style = SpeechCarefulStyleSpecification::known(false).unwrap();
    let syllable = SpeechSyllablePositionSpecification::unknown();
    let prosody = SpeechProsodicContextSpecification::unknown();
    let context =
        compare_declared_allophone_context(&declared, 1, explicit(&style, &syllable, &prosody))
            .unwrap();
    assert_eq!(
        context.decision(),
        &SpeechContextDecision::ObservationUnresolved
    );
    assert_eq!(
        context.neighbors().before().decision(),
        &SpeechNeighborDecision::ObservationUnresolved
    );
}
#[test]
fn syntax_refusal_remains_distinct_even_when_scalar_requirement_mismatches() {
    let intent = fixture::intent([
        fixture::segment(10),
        fixture::segment(11),
        fixture::segment(12),
    ]);
    let inventory = inventory(
        SpeechStress::Secondary,
        [
            SpeechRuleCondition::current_word_has_syntactic_link(SpeechSyntacticLinkKind::Vocative)
                .unwrap(),
        ],
    );
    let declared = resolve_declared_intent_realization(&intent, 1, &inventory).unwrap();
    let style = SpeechCarefulStyleSpecification::known(false).unwrap();
    let syllable = SpeechSyllablePositionSpecification::unknown();
    let prosody = SpeechProsodicContextSpecification::unknown();
    assert!(
        matches!(compare_declared_allophone_context(&declared,1,explicit(&style,&syllable,&prosody)),Err(DeclaredContextRefusal::Conditions(value)) if value.index == 0 && matches!(value.reason,ConditionRefusalReason::UnsupportedSyntax))
    );
}

#[test]
fn asymmetric_stress_conditions_use_the_same_exact_sides_as_matcher_context() {
    let inventory = inventory(
        SpeechStress::Primary,
        [
            SpeechRuleCondition::previous_stress(SpeechStress::Secondary).unwrap(),
            SpeechRuleCondition::next_stress(SpeechStress::Primary).unwrap(),
        ],
    );
    let style = SpeechCarefulStyleSpecification::known(false).unwrap();
    let syllable = SpeechSyllablePositionSpecification::unknown();
    let prosody = SpeechProsodicContextSpecification::unknown();
    for (before, after, expected) in [
        (
            SpeechStress::Secondary,
            SpeechStress::Primary,
            SpeechContextDecision::Matched,
        ),
        (
            SpeechStress::Primary,
            SpeechStress::Secondary,
            SpeechContextDecision::Mismatched,
        ),
    ] {
        let intent = fixture::intent([
            fixture::segment_stress(10, StressSpecification::known(before).unwrap()),
            fixture::segment(11),
            fixture::segment_stress(12, StressSpecification::known(after).unwrap()),
        ]);
        let declared = resolve_declared_intent_realization(&intent, 1, &inventory).unwrap();
        let context =
            compare_declared_allophone_context(&declared, 1, explicit(&style, &syllable, &prosody))
                .unwrap();
        assert_eq!(context.decision(), &expected);
        assert_eq!(context.conditions().receipts().count(), 2);
    }
}

#[test]
fn a_known_feature_on_the_definition_does_not_become_neighbor_observation() {
    let required = SpeechFeatureValue::boolean(true).unwrap();
    let key = SpeechFeatureId::new("fixture/feature".into()).unwrap();
    let initial = inventory(
        SpeechStress::Primary,
        [SpeechRuleCondition::previous_has_feature(key.clone(), required.clone()).unwrap()],
    );
    let definition = SpeechPhone::new(
        BoundedSequence::new(),
        SpeechFeatureBundle::new(
            BoundedSequence::try_from_iter([SpeechFeature::new(
                key,
                FeatureSpecification::known(required).unwrap(),
            )
            .unwrap()])
            .unwrap(),
        )
        .unwrap(),
        phone::id("phone/t"),
        "t".into(),
        SpeechSegmentStatus::Allophonic,
    )
    .unwrap();
    let inventory = SpeechInventory::new(
        initial.identity().clone(),
        initial.language().clone(),
        initial.phonemes().clone(),
        BoundedSequence::try_from_iter([definition]).unwrap(),
    )
    .unwrap();
    let intent = fixture::intent([
        fixture::segment(10),
        fixture::segment(11),
        fixture::segment(12),
    ]);
    let declared = resolve_declared_intent_realization(&intent, 1, &inventory).unwrap();
    let style = SpeechCarefulStyleSpecification::known(false).unwrap();
    let syllable = SpeechSyllablePositionSpecification::unknown();
    let prosody = SpeechProsodicContextSpecification::unknown();
    let context =
        compare_declared_allophone_context(&declared, 1, explicit(&style, &syllable, &prosody))
            .unwrap();
    assert_eq!(
        context.conditions().decision(),
        &SpeechContextDecision::ObservationUnresolved
    );
    assert_eq!(
        context.decision(),
        &SpeechContextDecision::ObservationUnresolved
    );
    let receipt = context.conditions().receipts().next().unwrap();
    assert!(
        matches!(receipt,conduit_speech::rule_conditions::ConditionReceipt::Feature(value) if value.feature().is_none())
    );
}
