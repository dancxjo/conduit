#![cfg(feature = "semantic-bindings")]
use conduit_core::IeeeF32;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{
    declared_context::ExplicitAllophoneContext, global_rule_selection::*, semantic::*,
};
#[allow(dead_code)]
#[path = "common/occurrence_intent.rs"]
mod fixture;
fn rule(
    id: &str,
    stress: StressSpecification,
    status: SpeechRuleStatus,
    phone: PhoneSpecification,
    syntax: bool,
) -> SpeechAllophoneRule {
    let conditions = if syntax {
        vec![
            SpeechRuleCondition::current_word_has_syntactic_link(SpeechSyntacticLinkKind::Subject)
                .unwrap(),
        ]
    } else {
        vec![]
    };
    SpeechAllophoneRule::new(
        BoundedSequence::try_from_iter(conditions).unwrap(),
        SpeechConfidence::new(IeeeF32::from_value(0.5)).unwrap(),
        SpeechEnvironment::new(
            BoundedSequence::new(),
            BoundedSequence::new(),
            SpeechProsodicContextSpecification::unspecified(),
            stress,
            SpeechSyllablePositionSpecification::unspecified(),
            SpeechPositionSpecification::unspecified(),
        )
        .unwrap(),
        id.into(),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        phone,
        PhonemeSpecification::known(PhonemeId::new("phoneme/t".into()).unwrap()).unwrap(),
        status,
    )
    .unwrap()
}
fn phone(id: &str) -> PhoneSpecification {
    PhoneSpecification::known(PhoneId::new(id.into()).unwrap()).unwrap()
}
fn matching(id: &str) -> SpeechAllophoneRule {
    rule(
        id,
        StressSpecification::unspecified(),
        SpeechRuleStatus::Productive,
        phone("phone/t"),
        false,
    )
}
fn profile(rules: Vec<SpeechAllophoneRule>) -> SpeechAllophoneRuleProfile {
    SpeechAllophoneRuleProfile::new(
        SpeechInventoryId::new(fixture::BASIS[0].into()).unwrap(),
        SpeechLanguageId::new(fixture::BASIS[1].into()).unwrap(),
        BoundedSequence::try_from_iter(rules).unwrap(),
    )
    .unwrap()
}
fn policy(experimental: bool) -> SpeechAllophoneChoicePolicy {
    SpeechAllophoneChoicePolicy::new(true, experimental, false, false, true, false).unwrap()
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
#[test]
fn first_permitted_match_retains_every_original_candidate_and_phone_constraint() {
    let intent = fixture::intent([fixture::segment(10)]);
    let profile = profile(vec![
        rule(
            "excluded",
            StressSpecification::unspecified(),
            SpeechRuleStatus::Experimental,
            phone("phone/t"),
            true,
        ),
        rule(
            "wrong phone",
            StressSpecification::unspecified(),
            SpeechRuleStatus::Productive,
            phone("phone/other"),
            false,
        ),
        matching("winner"),
        matching("later"),
    ]);
    let policy = policy(false);
    let explicit = Explicit::new();
    let choice =
        select_global_allophone_rule(&intent, 0, &profile, &policy, None, explicit.get()).unwrap();
    assert!(core::ptr::eq(choice.profile(), &profile));
    assert!(core::ptr::eq(choice.policy(), &policy));
    assert_eq!(
        choice.state().outcome(),
        &SpeechAllophoneChoiceOutcome::SelectedAllophone
    );
    assert_eq!(*choice.state().index(), 2);
    assert!(core::ptr::eq(
        choice.selected_rule().unwrap(),
        &profile.rules().as_slice()[2]
    ));
    assert_eq!(choice.candidates().len(), 4);
    let excluded = &choice.candidates()[0];
    assert!(!excluded.status_allowed());
    assert!(excluded.evaluation().is_none());
    assert!(excluded.phone_constraint().is_none());
    assert!(excluded.decision().is_none());
    assert_eq!(
        choice.candidates()[1].decision(),
        Some(&SpeechContextDecision::Mismatched)
    );
    for (index, candidate) in choice.candidates().iter().enumerate() {
        assert!(core::ptr::eq(
            candidate.rule(),
            &profile.rules().as_slice()[index]
        ));
        if index > 0 {
            assert!(core::ptr::eq(
                candidate
                    .evaluation()
                    .unwrap()
                    .context()
                    .occurrence()
                    .segment(),
                choice.occurrence().segment()
            ));
        }
    }
    assert_eq!(
        choice.candidates()[3].decision(),
        Some(&SpeechContextDecision::Matched)
    );
}
#[test]
fn earlier_unresolved_requirements_and_phone_observations_block_lower_matches() {
    let intent = fixture::intent([fixture::segment(10)]);
    let policy = policy(false);
    let explicit = Explicit::new();
    for (stress, phone, reason) in [
        (
            StressSpecification::unknown(),
            phone("phone/t"),
            SpeechContextDecision::RequirementUnresolved,
        ),
        (
            StressSpecification::unspecified(),
            PhoneSpecification::unknown(),
            SpeechContextDecision::ObservationUnresolved,
        ),
    ] {
        let profile = profile(vec![
            rule(
                "blocker",
                stress,
                SpeechRuleStatus::Productive,
                phone,
                false,
            ),
            matching("lower"),
        ]);
        let choice =
            select_global_allophone_rule(&intent, 0, &profile, &policy, None, explicit.get())
                .unwrap();
        assert_eq!(
            choice.state().outcome(),
            &SpeechAllophoneChoiceOutcome::Deferred
        );
        assert_eq!(*choice.state().index(), 0);
        assert_eq!(choice.state().reason(), &reason);
        assert!(choice.selected_rule().is_none());
        assert_eq!(
            choice.candidates()[1].decision(),
            Some(&SpeechContextDecision::Matched)
        );
    }
}
#[test]
fn later_unsupported_obligations_refuse_and_experimental_permission_is_explicit() {
    let intent = fixture::intent([fixture::segment(10)]);
    let explicit = Explicit::new();
    let profile = profile(vec![
        matching("winner"),
        rule(
            "syntax",
            StressSpecification::unspecified(),
            SpeechRuleStatus::Experimental,
            phone("phone/t"),
            true,
        ),
    ]);
    assert!(select_global_allophone_rule(
        &intent,
        0,
        &profile,
        &policy(false),
        None,
        explicit.get()
    )
    .is_ok());
    assert!(matches!(
        select_global_allophone_rule(&intent, 0, &profile, &policy(true), None, explicit.get()),
        Err(GlobalRuleSelectionRefusal::Evaluation { index: 1, .. })
    ));
}
#[test]
fn empty_and_full_profiles_preserve_bounds_without_inventing_default_selection() {
    let intent = fixture::intent([fixture::segment(10)]);
    let policy = policy(false);
    let explicit = Explicit::new();
    let empty = profile(vec![]);
    let choice =
        select_global_allophone_rule(&intent, 0, &empty, &policy, None, explicit.get()).unwrap();
    assert_eq!(
        choice.state().outcome(),
        &SpeechAllophoneChoiceOutcome::None
    );
    assert!(choice.selected_rule().is_none());
    assert!(choice.candidates().is_empty());
    let full = profile((0..8).map(|n| matching(&format!("rule/{n}"))).collect());
    assert_eq!(
        select_global_allophone_rule(&intent, 0, &full, &policy, None, explicit.get())
            .unwrap()
            .candidates()
            .len(),
        8
    );
    assert!(BoundedSequence::<SpeechAllophoneRule, 8>::try_from_iter(
        (0..9).map(|n| matching(&format!("rule/{n}")))
    )
    .is_err());
}
#[test]
fn foreign_feature_occurrence_refuses_even_when_profile_is_empty() {
    let intent = fixture::intent([fixture::segment(10)]);
    let profile = profile(vec![]);
    let policy = policy(false);
    let explicit = Explicit::new();
    let observed = SpeechOccurrenceFeatureObservation::new(
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        fixture::token(11, fixture::BASIS),
        SpeechEvidenceProvenance::new(
            "foreign feature fixture".into(),
            SpeechEvidenceSource::Manual,
            None,
        )
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(
        select_global_allophone_rule(
            &intent,
            0,
            &profile,
            &policy,
            Some(&observed),
            explicit.get()
        ),
        Err(GlobalRuleSelectionRefusal::FeatureOccurrence(_))
    ));
    let exact = SpeechOccurrenceFeatureObservation::new(
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        fixture::token(10, fixture::BASIS),
        observed.provenance().clone(),
    )
    .unwrap();
    let choice =
        select_global_allophone_rule(&intent, 0, &profile, &policy, Some(&exact), explicit.get())
            .unwrap();
    assert!(core::ptr::eq(choice.observed_features().unwrap(), &exact));
    assert!(choice.checked_features().is_some());
}

#[test]
fn foreign_profile_basis_and_invalid_feature_evidence_refuse_before_empty_choice() {
    let intent = fixture::intent([fixture::segment(10)]);
    let policy = policy(false);
    let explicit = Explicit::new();
    for (inventory, language) in [
        ("foreign", fixture::BASIS[1]),
        (fixture::BASIS[0], "foreign"),
    ] {
        let profile = SpeechAllophoneRuleProfile::new(
            SpeechInventoryId::new(inventory.into()).unwrap(),
            SpeechLanguageId::new(language.into()).unwrap(),
            BoundedSequence::new(),
        )
        .unwrap();
        assert!(matches!(
            select_global_allophone_rule(&intent, 0, &profile, &policy, None, explicit.get()),
            Err(GlobalRuleSelectionRefusal::Basis(_))
        ));
    }
    let duplicate = SpeechFeature::new(
        SpeechFeatureId::new("voice".into()).unwrap(),
        FeatureSpecification::unknown(),
    )
    .unwrap();
    let observed = SpeechOccurrenceFeatureObservation::new(
        SpeechFeatureBundle::new(
            BoundedSequence::try_from_iter([duplicate.clone(), duplicate]).unwrap(),
        )
        .unwrap(),
        fixture::token(10, fixture::BASIS),
        SpeechEvidenceProvenance::new(
            "invalid feature fixture".into(),
            SpeechEvidenceSource::Manual,
            None,
        )
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(
        select_global_allophone_rule(
            &intent,
            0,
            &profile(vec![]),
            &policy,
            Some(&observed),
            explicit.get()
        ),
        Err(GlobalRuleSelectionRefusal::FeatureEvidence(_))
    ));
}
#[test]
fn unresolved_requested_phone_is_refused_but_unconstrained_selection_is_not_realization() {
    let SpeechUtteranceIntentEvent::Segment(segment) = fixture::segment(10) else {
        unreachable!()
    };
    let profile = profile(vec![rule(
        "unknown output",
        StressSpecification::unspecified(),
        SpeechRuleStatus::Productive,
        PhoneSpecification::unknown(),
        false,
    )]);
    let policy = policy(false);
    let explicit = Explicit::new();
    for requested in [
        PhoneSpecification::unknown(),
        PhoneSpecification::not_applicable(),
        PhoneSpecification::variable(
            BoundedSequence::try_from_iter([PhoneId::new("phone/t".into()).unwrap()]).unwrap(),
        )
        .unwrap(),
        PhoneSpecification::gradient(
            SpeechConfidence::new(IeeeF32::from_value(0.5)).unwrap(),
            PhoneId::new("phone/t".into()).unwrap(),
        )
        .unwrap(),
        PhoneSpecification::unspecified(),
    ] {
        let intent = fixture::intent([SpeechUtteranceIntentEvent::segment(
            segment.occurrence().clone(),
            requested.clone(),
            segment.phoneme().clone(),
            segment.prosody().clone(),
            segment.provenance().clone(),
            segment.sources().clone(),
            segment.stress().clone(),
            segment.word_position().clone(),
        )
        .unwrap()]);
        let result =
            select_global_allophone_rule(&intent, 0, &profile, &policy, None, explicit.get());
        if matches!(requested, PhoneSpecification::Unspecified) {
            let choice = result.unwrap();
            assert_eq!(
                choice.state().outcome(),
                &SpeechAllophoneChoiceOutcome::SelectedAllophone
            );
            assert!(matches!(
                choice.selected_rule().unwrap().phone(),
                PhoneSpecification::Unknown
            ));
        } else {
            assert!(matches!(
                result,
                Err(GlobalRuleSelectionRefusal::UnresolvedPhone(_))
            ));
        }
    }
}
