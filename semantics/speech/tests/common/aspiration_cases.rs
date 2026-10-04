//! Whole-intent feature lowering uses the existing admission fixture.
use super::*;
use conduit_speech::feature_realization::FeatureRealizationRefusal;
fn features(key: &str, value: FeatureSpecification) -> SpeechFeatureBundle {
    SpeechFeatureBundle::new(
        BoundedSequence::try_from_iter([SpeechFeature::new(
            SpeechFeatureId::new(key.into()).unwrap(),
            value,
        )
        .unwrap()])
        .unwrap(),
    )
    .unwrap()
}
fn observation(ordinal: u32, bundle: SpeechFeatureBundle) -> SpeechOccurrenceFeatureObservation {
    let original = default(ordinal);
    SpeechOccurrenceFeatureObservation::new(
        bundle,
        original.occurrence().clone(),
        original.provenance().clone(),
    )
    .unwrap()
}
fn profile(setup: &Setup) -> SpeechFormantAspirationProfile {
    SpeechFormantAspirationProfile::new(
        SpeechFeatureId::new("opaque cue 7".into()).unwrap(),
        setup.voice.clone(),
    )
    .unwrap()
}
#[test]
fn explicit_known_boolean_changes_acoustics_and_retains_original_feature_and_phone() {
    let setup = Setup::new();
    let profile = profile(&setup);
    let source = occurrence::intent([segment(10)]);
    for enabled in [false, true] {
        let defaults = observation(
            10,
            features(
                "opaque cue 7",
                FeatureSpecification::known(SpeechFeatureValue::boolean(enabled).unwrap()).unwrap(),
            ),
        );
        let evidence = [setup.evidence(&defaults)];
        let prepared = prepare_aspirated_global_intent(
            &source,
            &setup.inventory,
            &profile,
            &setup.boundaries,
            &setup.rules,
            &setup.policy,
            &evidence,
        )
        .unwrap();
        let receipt = &prepared.phones()[0];
        assert_eq!(receipt.definition().identity(), &native::id("phone/t"));
        let acoustic = receipt.aspiration().unwrap();
        assert!(core::ptr::eq(acoustic.profile(), &profile));
        assert!(core::ptr::eq(
            acoustic.feature().unwrap(),
            &defaults.features().get().as_slice()[0]
        ));
        assert!(acoustic.checked_identity().is_some());
        let expected = VoiceEvent::phone(SpeechPhoneInput {
            phone: if enabled {
                conduit_speech::EnglishPhone::t_aspirated
            } else {
                conduit_speech::EnglishPhone::t
            },
            stress: EnglishStress::primary,
        });
        assert_eq!(prepared.events()[0], expected);
        assert_eq!(
            pcm(prepared.renderer().unwrap(), 1),
            pcm(prepared.renderer().unwrap(), 128)
        );
        assert!(matches!(
            prepare_global_intent(
                &source,
                &setup.inventory,
                &setup.voice,
                &setup.boundaries,
                &setup.rules,
                &setup.policy,
                &evidence
            ),
            Err(GlobalIntentRefusal::Profile { .. })
        ));
    }
}
#[test]
fn later_unsupported_key_and_unresolved_value_refuse_entire_intent_at_exact_event() {
    let setup = Setup::new();
    let profile = profile(&setup);
    let source = occurrence::intent([segment(10), segment(11)]);
    let first = observation(
        10,
        features(
            "opaque cue 7",
            FeatureSpecification::known(SpeechFeatureValue::boolean(true).unwrap()).unwrap(),
        ),
    );
    let value = SpeechFeatureValue::boolean(false).unwrap();
    let states = [
        FeatureSpecification::unknown(),
        FeatureSpecification::unspecified(),
        FeatureSpecification::not_applicable(),
        FeatureSpecification::variable(BoundedSequence::try_from_iter([value.clone()]).unwrap())
            .unwrap(),
        FeatureSpecification::gradient(
            SpeechConfidence::new(IeeeF32::from_value(0.5)).unwrap(),
            value,
        )
        .unwrap(),
    ];
    for state in states {
        let second = observation(11, features("opaque cue 7", state));
        let result = prepare_aspirated_global_intent(
            &source,
            &setup.inventory,
            &profile,
            &setup.boundaries,
            &setup.rules,
            &setup.policy,
            &[setup.evidence(&first), setup.evidence(&second)],
        );
        assert!(
            matches!(result, Err(GlobalIntentRefusal::Profile { event: 1, reason: ChosenGlobalProfileRefusal::AcousticFeature(FeatureRealizationRefusal::Unresolved(spec)) }) if core::ptr::eq(spec, second.features().get().as_slice()[0].specification()))
        );
    }
    for (key, state) in [
        (
            "unmapped",
            FeatureSpecification::known(SpeechFeatureValue::boolean(true).unwrap()).unwrap(),
        ),
        (
            "opaque cue 7",
            FeatureSpecification::known(SpeechFeatureValue::text("true".into()).unwrap()).unwrap(),
        ),
    ] {
        let second = observation(11, features(key, state));
        assert!(matches!(
            prepare_aspirated_global_intent(
                &source,
                &setup.inventory,
                &profile,
                &setup.boundaries,
                &setup.rules,
                &setup.policy,
                &[setup.evidence(&first), setup.evidence(&second)]
            ),
            Err(GlobalIntentRefusal::Profile {
                event: 1,
                reason: ChosenGlobalProfileRefusal::AcousticFeature(_)
            })
        ));
    }
}

#[test]
fn rule_override_is_realized_as_a_whole_specification_and_all_keys_need_coverage() {
    let mut setup = Setup::new();
    let original = &setup.rules.rules().as_slice()[0];
    let rule = SpeechAllophoneRule::new(
        original.conditions().clone(),
        original.confidence().clone(),
        original.environment().clone(),
        original.identity().clone(),
        original.input_features().clone(),
        features(
            "opaque cue 7",
            FeatureSpecification::known(SpeechFeatureValue::boolean(true).unwrap()).unwrap(),
        ),
        original.phone().clone(),
        original.phoneme().clone(),
        original.status().clone(),
    )
    .unwrap();
    setup.rules = SpeechAllophoneRuleProfile::new(
        setup.rules.inventory_id().clone(),
        setup.rules.language().clone(),
        BoundedSequence::try_from_iter([rule]).unwrap(),
    )
    .unwrap();
    let profile = profile(&setup);
    let source = occurrence::intent([segment(10)]);
    let defaults = observation(
        10,
        features("opaque cue 7", FeatureSpecification::unknown()),
    );
    let prepared = prepare_aspirated_global_intent(
        &source,
        &setup.inventory,
        &profile,
        &setup.boundaries,
        &setup.rules,
        &setup.policy,
        &[setup.evidence(&defaults)],
    )
    .unwrap();
    let receipt = &prepared.phones()[0];
    assert_eq!(
        receipt
            .features()
            .unwrap()
            .features()
            .next()
            .unwrap()
            .layer(),
        conduit_speech::output_features::OutputFeatureLayer::RuleOutput
    );
    assert!(core::ptr::eq(
        receipt.aspiration().unwrap().feature().unwrap(),
        &setup.rules.rules().as_slice()[0]
            .output_features()
            .get()
            .as_slice()[0]
    ));
    assert!(matches!(
        prepared.events()[0],
        VoiceEvent::phone(SpeechPhoneInput {
            phone: conduit_speech::EnglishPhone::t_aspirated,
            ..
        })
    ));
    let bundle = SpeechFeatureBundle::new(
        BoundedSequence::try_from_iter([
            SpeechFeature::new(
                SpeechFeatureId::new("opaque cue 7".into()).unwrap(),
                FeatureSpecification::known(SpeechFeatureValue::boolean(false).unwrap()).unwrap(),
            )
            .unwrap(),
            SpeechFeature::new(
                SpeechFeatureId::new("unmapped".into()).unwrap(),
                FeatureSpecification::unknown(),
            )
            .unwrap(),
        ])
        .unwrap(),
    )
    .unwrap();
    let defaults = observation(10, bundle);
    assert!(
        matches!(prepare_aspirated_global_intent(&source, &setup.inventory, &profile, &setup.boundaries, &setup.rules, &setup.policy, &[setup.evidence(&defaults)]), Err(GlobalIntentRefusal::Profile {event: 0, reason: ChosenGlobalProfileRefusal::AcousticFeature(FeatureRealizationRefusal::UnsupportedFeature(feature))}) if core::ptr::eq(feature, &defaults.features().get().as_slice()[1]))
    );
}
