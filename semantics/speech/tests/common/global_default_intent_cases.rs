use super::*;
use conduit_speech::global_default_profile::GlobalDefaultProfileRefusal;
#[test]
fn mixed_rules_defaults_and_boundary_are_admitted_as_one_immutable_intent() {
    let SpeechUtteranceIntentEvent::Segment(second) = segment(11) else {
        unreachable!()
    };
    let second = SpeechUtteranceIntentEvent::segment(
        second.occurrence().clone(),
        second.phone().clone(),
        second.phoneme().clone(),
        second.prosody().clone(),
        second.provenance().clone(),
        second.sources().clone(),
        StressSpecification::known(SpeechStress::Unstressed).unwrap(),
        second.word_position().clone(),
    )
    .unwrap();
    let source = occurrence::intent([segment(10), boundary(), second]);
    let mut setup = Setup::new();
    setup.policy =
        SpeechAllophoneChoicePolicy::new(true, false, false, false, true, false).unwrap();
    // Primary first segment matches the rule; unstressed second uses its default.
    setup.rules = rules(StressSpecification::known(SpeechStress::Primary).unwrap());
    let defaults = [default(10), default(11)];
    let evidence = [
        setup.evidence(&defaults[0]),
        None,
        setup.evidence(&defaults[1]),
    ];
    let prepared = prepare_global_intent(
        &source,
        &setup.inventory,
        &setup.voice,
        &setup.boundaries,
        &setup.rules,
        &setup.policy,
        &evidence,
    )
    .unwrap();
    assert!(core::ptr::eq(prepared.source(), &source));
    let rule = &prepared.phones()[0];
    assert_eq!(
        rule.completed_choice().outcome(),
        &SpeechAllophoneChoiceOutcome::SelectedAllophone
    );
    assert!(matches!(
        rule.realization(),
        GlobalIntentRealization::Rule(_)
    ));
    assert!(rule.features().is_some());
    let receipt = &prepared.phones()[1];
    assert_eq!(
        receipt.choice().state().outcome(),
        &SpeechAllophoneChoiceOutcome::None
    );
    assert_eq!(
        receipt.completed_choice().outcome(),
        &SpeechAllophoneChoiceOutcome::SelectedDefault
    );
    assert!(matches!(
        receipt.realization(),
        GlobalIntentRealization::Default { .. }
    ));
    assert!(receipt.features().is_none());
    assert_eq!(
        pcm(prepared.renderer().unwrap(), 1),
        pcm(prepared.renderer().unwrap(), 128)
    );
    let material = LanguageText::new(
        LanguageTextId::new("source".into()).unwrap(),
        source.language().clone(),
        LanguageTextRevisionId::new("source revision".into()).unwrap(),
        "t".into(),
    )
    .unwrap();
    let sourced = conduit_speech::sourced_global_intent::prepare_sourced_global_intent(
        &source,
        &[conduit_speech::intent_sources::IntentSourceMaterial::Text(&material); 3],
        &setup.inventory,
        &setup.voice,
        &setup.boundaries,
        &setup.rules,
        &setup.policy,
        &evidence,
    )
    .unwrap();
    assert!(core::ptr::eq(
        sourced.sources().intent(),
        sourced.realization().source()
    ));
    assert_eq!(sourced.sources().receipts().len(), 3);
    assert!(matches!(
        sourced.realization().phones()[0].realization(),
        GlobalIntentRealization::Rule(_)
    ));
    assert!(matches!(
        sourced.realization().phones()[1].realization(),
        GlobalIntentRealization::Default { .. }
    ));
    assert_eq!(
        pcm(sourced.renderer().unwrap(), 1),
        pcm(prepared.renderer().unwrap(), 128)
    );
    let late = default(99);
    assert!(matches!(
        prepare_global_intent(
            &source,
            &setup.inventory,
            &setup.voice,
            &setup.boundaries,
            &setup.rules,
            &setup.policy,
            &[setup.evidence(&defaults[0]), None, setup.evidence(&late)]
        ),
        Err(GlobalIntentRefusal::DefaultProfile {
            event: 2,
            reason: GlobalDefaultProfileRefusal::DefaultOccurrence(_)
        })
    ));
}
#[test]
fn default_permission_never_replaces_earlier_deferral() {
    let source = occurrence::intent([segment(10)]);
    let mut setup = Setup::new();
    setup.policy =
        SpeechAllophoneChoicePolicy::new(true, false, false, false, true, false).unwrap();
    setup.rules = rules(StressSpecification::unknown());
    let defaults = default(10);
    assert!(
        matches!(prepare_global_intent(&source, &setup.inventory, &setup.voice, &setup.boundaries, &setup.rules, &setup.policy, &[setup.evidence(&defaults)]),
        Err(GlobalIntentRefusal::Profile { event: 0, reason: ChosenGlobalProfileRefusal::Unchosen(state) })
        if matches!(state.outcome(), SpeechAllophoneChoiceOutcome::Deferred))
    );
}
