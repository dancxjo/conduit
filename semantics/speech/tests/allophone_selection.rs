#![cfg(feature = "semantic-bindings")]
use conduit_core::IeeeF32;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{
    allophone_selection::*, chosen_allophone_profile::*,
    declared_context::ExplicitAllophoneContext, semantic::*,
};
#[allow(dead_code)]
#[path = "common/occurrence_intent.rs"]
mod fixture;
fn intent(phone: PhoneSpecification) -> SpeechUtteranceIntent {
    let SpeechUtteranceIntentEvent::Segment(value) = fixture::segment(10) else {
        unreachable!()
    };
    fixture::intent([SpeechUtteranceIntentEvent::segment(
        value.occurrence().clone(),
        phone,
        value.phoneme().clone(),
        value.prosody().clone(),
        value.provenance().clone(),
        value.sources().clone(),
        value.stress().clone(),
        value.word_position().clone(),
    )
    .unwrap()])
}
fn rule(
    phone: &str,
    status: SpeechRuleStatus,
    stress: StressSpecification,
    conditions: Vec<SpeechRuleCondition>,
) -> SpeechPhonemeAllophone {
    SpeechPhonemeAllophone::new(
        BoundedSequence::try_from_iter(conditions).unwrap(),
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
        PhoneId::new(phone.into()).unwrap(),
        Some("original rule".into()),
        status,
    )
    .unwrap()
}
fn definition(id: &str, ipa: &str) -> SpeechPhone {
    SpeechPhone::new(
        BoundedSequence::new(),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        PhoneId::new(id.into()).unwrap(),
        ipa.into(),
        SpeechSegmentStatus::Allophonic,
    )
    .unwrap()
}
fn inventory(rules: Vec<SpeechPhonemeAllophone>, default: Option<&str>) -> SpeechInventory {
    let phoneme = SpeechPhoneme::new(
        BoundedSequence::new(),
        BoundedSequence::try_from_iter(rules).unwrap(),
        default.map(|id| PhoneId::new(id.into()).unwrap()),
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
        BoundedSequence::try_from_iter([definition("phone/t", "t"), definition("phone/alt", "tʰ")])
            .unwrap(),
    )
    .unwrap()
}
fn policy(mask: u8, fallback: bool) -> SpeechAllophoneChoicePolicy {
    SpeechAllophoneChoicePolicy::new(
        fallback,
        mask & 16 != 0,
        mask & 2 != 0,
        mask & 4 != 0,
        mask & 1 != 0,
        mask & 8 != 0,
    )
    .unwrap()
}
struct Extra {
    style: SpeechCarefulStyleSpecification,
    syllable: SpeechSyllablePositionSpecification,
    prosody: SpeechProsodicContextSpecification,
}
impl Extra {
    fn new() -> Self {
        Self {
            style: SpeechCarefulStyleSpecification::known(false).unwrap(),
            syllable: SpeechSyllablePositionSpecification::unknown(),
            prosody: SpeechProsodicContextSpecification::unknown(),
        }
    }
    fn context(&self) -> ExplicitAllophoneContext<'_> {
        ExplicitAllophoneContext {
            careful_style: &self.style,
            syllable_position: &self.syllable,
            prosodic_context: &self.prosody,
        }
    }
}
#[test]
fn all_status_masks_are_explicit_and_keep_original_declarations() {
    let intent = intent(PhoneSpecification::unspecified());
    let extra = Extra::new();
    for (bit, status) in [
        SpeechRuleStatus::Productive,
        SpeechRuleStatus::Lexicalized,
        SpeechRuleStatus::Optional,
        SpeechRuleStatus::StyleDependent,
        SpeechRuleStatus::Experimental,
    ]
    .into_iter()
    .enumerate()
    {
        let inventory = inventory(
            vec![rule(
                "phone/alt",
                status,
                StressSpecification::unspecified(),
                vec![],
            )],
            Some("phone/t"),
        );
        for mask in 0..32 {
            let policy = policy(mask, true);
            let choice =
                select_intent_allophone(&intent, 0, &inventory, &policy, extra.context()).unwrap();
            let allowed = mask & (1 << bit) != 0;
            assert_eq!(
                choice.state().outcome(),
                if allowed {
                    &SpeechAllophoneChoiceOutcome::SelectedAllophone
                } else {
                    &SpeechAllophoneChoiceOutcome::SelectedDefault
                }
            );
            let candidate = choice.candidates().next().unwrap();
            assert!(core::ptr::eq(
                candidate.declaration(),
                &inventory.phonemes().as_slice()[0].allophones().as_slice()[0]
            ));
            assert_eq!(candidate.status_allowed(), allowed);
            assert_eq!(candidate.context_decision().is_some(), allowed);
            assert!(core::ptr::eq(choice.policy(), &policy));
        }
    }
}
#[test]
fn first_eligible_choice_keeps_every_receipt_and_full_bound() {
    let intent = intent(PhoneSpecification::unspecified());
    let inventory = inventory(
        (0..8)
            .map(|index| {
                rule(
                    if index == 0 { "phone/alt" } else { "phone/t" },
                    SpeechRuleStatus::Productive,
                    StressSpecification::unspecified(),
                    vec![],
                )
            })
            .collect(),
        Some("phone/t"),
    );
    let policy = policy(31, true);
    let extra = Extra::new();
    let choice = select_intent_allophone(&intent, 0, &inventory, &policy, extra.context()).unwrap();
    assert_eq!(choice.state().index(), &0);
    assert_eq!(choice.selected_phone().unwrap().get(), "phone/alt");
    assert_eq!(choice.candidates().count(), 8);
    assert!(choice
        .candidates()
        .all(|value| value.context_decision() == Some(&SpeechContextDecision::Matched)));
    assert!(BoundedSequence::<_, 8>::try_from_iter((0..9).map(|_| rule(
        "phone/t",
        SpeechRuleStatus::Productive,
        StressSpecification::unspecified(),
        vec![]
    )))
    .is_err());
}
#[test]
fn a_higher_priority_unknown_blocks_lower_matches_and_default() {
    let intent = intent(PhoneSpecification::unspecified());
    let policy = policy(31, true);
    let extra = Extra::new();
    for required in [true, false] {
        let blocking = if required {
            rule(
                "phone/alt",
                SpeechRuleStatus::Productive,
                StressSpecification::unknown(),
                vec![],
            )
        } else {
            rule(
                "phone/alt",
                SpeechRuleStatus::Productive,
                StressSpecification::unspecified(),
                vec![SpeechRuleCondition::previous_stress(SpeechStress::Primary).unwrap()],
            )
        };
        let inventory = inventory(
            vec![
                blocking,
                rule(
                    "phone/t",
                    SpeechRuleStatus::Productive,
                    StressSpecification::unspecified(),
                    vec![],
                ),
            ],
            Some("phone/t"),
        );
        let choice =
            select_intent_allophone(&intent, 0, &inventory, &policy, extra.context()).unwrap();
        assert_eq!(
            choice.state().outcome(),
            &SpeechAllophoneChoiceOutcome::Deferred
        );
        assert_eq!(
            choice.state().reason(),
            if required {
                &SpeechContextDecision::RequirementUnresolved
            } else {
                &SpeechContextDecision::ObservationUnresolved
            }
        );
        assert_eq!(
            choice.candidates().nth(1).unwrap().context_decision(),
            Some(&SpeechContextDecision::Matched)
        );
        assert!(choice.selected_phone().is_none());
    }
}
#[test]
fn mismatch_exclusion_default_permission_and_missing_default_are_distinct() {
    let intent = intent(PhoneSpecification::unspecified());
    let extra = Extra::new();
    for (fallback, default, expected) in [
        (
            true,
            Some("phone/t"),
            SpeechAllophoneChoiceOutcome::SelectedDefault,
        ),
        (false, Some("phone/t"), SpeechAllophoneChoiceOutcome::None),
        (true, None, SpeechAllophoneChoiceOutcome::None),
    ] {
        let inventory = inventory(
            vec![rule(
                "phone/alt",
                SpeechRuleStatus::Productive,
                StressSpecification::known(SpeechStress::Secondary).unwrap(),
                vec![],
            )],
            default,
        );
        let policy = policy(31, fallback);
        let choice =
            select_intent_allophone(&intent, 0, &inventory, &policy, extra.context()).unwrap();
        assert_eq!(choice.state().outcome(), &expected);
        assert_eq!(
            choice.candidates().next().unwrap().context_decision(),
            Some(&SpeechContextDecision::Mismatched)
        );
    }
}
#[test]
fn known_phone_constraints_are_never_replaced_and_unresolved_requests_are_retained() {
    let extra = Extra::new();
    let policy = policy(31, true);
    let inventory = inventory(
        vec![rule(
            "phone/alt",
            SpeechRuleStatus::Productive,
            StressSpecification::unspecified(),
            vec![],
        )],
        Some("phone/t"),
    );
    let requested =
        intent(PhoneSpecification::known(PhoneId::new("phone/t".into()).unwrap()).unwrap());
    let choice =
        select_intent_allophone(&requested, 0, &inventory, &policy, extra.context()).unwrap();
    assert_eq!(
        choice.state().outcome(),
        &SpeechAllophoneChoiceOutcome::SelectedDefault
    );
    assert!(!choice.candidates().next().unwrap().eligible());
    assert!(choice
        .candidates()
        .next()
        .unwrap()
        .phone_constraint()
        .unwrap()
        .is_err());
    for specification in [
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
    ] {
        let requested = intent(specification);
        assert!(
            matches!(select_intent_allophone(&requested,0,&inventory,&policy,extra.context()),Err(AllophoneSelectionRefusal::UnresolvedPhone(value)) if core::ptr::eq(value,match &requested.events().as_slice()[0] {SpeechUtteranceIntentEvent::Segment(value)=>value.phone(),_=>unreachable!()}))
        );
    }
}
#[test]
fn an_excluded_unsupported_rule_is_not_compared_but_an_allowed_one_refuses() {
    let intent = intent(PhoneSpecification::unspecified());
    let extra = Extra::new();
    let inventory = inventory(
        vec![rule(
            "phone/alt",
            SpeechRuleStatus::Experimental,
            StressSpecification::unspecified(),
            vec![SpeechRuleCondition::current_word_has_syntactic_link(
                SpeechSyntacticLinkKind::Vocative,
            )
            .unwrap()],
        )],
        Some("phone/t"),
    );
    let excluded = policy(1, true);
    let choice =
        select_intent_allophone(&intent, 0, &inventory, &excluded, extra.context()).unwrap();
    assert_eq!(
        choice.state().outcome(),
        &SpeechAllophoneChoiceOutcome::SelectedDefault
    );
    assert!(choice.candidates().next().unwrap().conditions().is_none());
    let allowed = policy(31, true);
    assert!(matches!(
        select_intent_allophone(&intent, 0, &inventory, &allowed, extra.context()),
        Err(AllophoneSelectionRefusal::Context { index: 0, .. })
    ));
}
#[test]
fn selected_phone_lowers_through_exact_profile_without_rewriting_intent() {
    let intent = intent(PhoneSpecification::unspecified());
    let extra = Extra::new();
    let policy = policy(31, true);
    let inventory = inventory(
        vec![rule(
            "phone/alt",
            SpeechRuleStatus::Productive,
            StressSpecification::unspecified(),
            vec![],
        )],
        Some("phone/t"),
    );
    let choice = select_intent_allophone(&intent, 0, &inventory, &policy, extra.context()).unwrap();
    let profile = SpeechFormantVoiceProfile::new(
        "choice profile".into(),
        inventory.identity().clone(),
        inventory.language().clone(),
        BoundedSequence::try_from_iter([SpeechFormantPhoneBinding::new(
            inventory.phones().as_slice()[1].clone(),
            EnglishPhone::TAspirated,
        )
        .unwrap()])
        .unwrap(),
    )
    .unwrap();
    let prepared = prepare_chosen_allophone_profile(&choice, &profile).unwrap();
    assert!(core::ptr::eq(prepared.choice(), &choice));
    assert_eq!(
        prepared.definition().identity(),
        choice.selected_phone().unwrap()
    );
    assert!(
        matches!(prepared.event(), conduit_speech::VoiceEvent::phone(input) if input.phone == conduit_speech::EnglishPhone::t_aspirated)
    );
    assert!(matches!(
        choice.occurrence().segment().phone(),
        PhoneSpecification::Unspecified
    ));
}

#[test]
fn native_receipt_admits_only_the_26_meaningful_choice_states() {
    let mut accepted = 0;
    for outcome in [
        SpeechAllophoneChoiceOutcome::None,
        SpeechAllophoneChoiceOutcome::SelectedAllophone,
        SpeechAllophoneChoiceOutcome::Deferred,
        SpeechAllophoneChoiceOutcome::SelectedDefault,
    ] {
        for index in 0..8 {
            for reason in [
                SpeechContextDecision::Matched,
                SpeechContextDecision::Mismatched,
                SpeechContextDecision::RequirementUnresolved,
                SpeechContextDecision::ObservationUnresolved,
            ] {
                let valid = match outcome {
                    SpeechAllophoneChoiceOutcome::None => {
                        index == 0 && reason == SpeechContextDecision::Mismatched
                    }
                    SpeechAllophoneChoiceOutcome::SelectedDefault => {
                        index == 0 && reason == SpeechContextDecision::Matched
                    }
                    SpeechAllophoneChoiceOutcome::SelectedAllophone => {
                        reason == SpeechContextDecision::Matched
                    }
                    SpeechAllophoneChoiceOutcome::Deferred => matches!(
                        reason,
                        SpeechContextDecision::RequirementUnresolved
                            | SpeechContextDecision::ObservationUnresolved
                    ),
                };
                assert_eq!(
                    SpeechAllophoneChoiceState::new(index, outcome, reason).is_ok(),
                    valid
                );
                accepted += usize::from(valid);
            }
        }
    }
    assert_eq!(accepted, 26);
    assert!(SpeechAllophoneChoiceState::new(
        8,
        SpeechAllophoneChoiceOutcome::SelectedAllophone,
        SpeechContextDecision::Matched
    )
    .is_err());
}
