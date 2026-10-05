//! Rule and declared-default feature laws remain distinct in acoustic admission.
use super::aspiration_cases::{features, observation, profile};
use super::*;
use conduit_speech::{
    feature_realization::FeatureRealizationRefusal,
    global_default_profile::GlobalDefaultProfileRefusal, output_features::OutputFeatureLayer,
};
fn boolean(value: bool) -> FeatureSpecification {
    FeatureSpecification::known(SpeechFeatureValue::boolean(value).unwrap()).unwrap()
}
fn definition_features(setup: &mut Setup, bundle: SpeechFeatureBundle) {
    let old = &setup.inventory.phones().as_slice()[0];
    let phone = SpeechPhone::new(
        old.aliases().clone(),
        bundle,
        old.identity().clone(),
        old.ipa().clone(),
        *old.status(),
    )
    .unwrap();
    setup.inventory = SpeechInventory::new(
        setup.inventory.identity().clone(),
        setup.inventory.language().clone(),
        setup.inventory.phonemes().clone(),
        BoundedSequence::try_from_iter([phone.clone()]).unwrap(),
    )
    .unwrap();
    setup.voice = SpeechFormantVoiceProfile::new(
        setup.voice.identity().clone(),
        setup.voice.inventory_id().clone(),
        setup.voice.language().clone(),
        BoundedSequence::try_from_iter([
            SpeechFormantPhoneBinding::new(phone, EnglishPhone::T).unwrap()
        ])
        .unwrap(),
    )
    .unwrap();
}
fn defaults_only(setup: &mut Setup) {
    setup.rules = SpeechAllophoneRuleProfile::new(
        setup.rules.inventory_id().clone(),
        setup.rules.language().clone(),
        BoundedSequence::new(),
    )
    .unwrap();
    setup.policy =
        SpeechAllophoneChoicePolicy::new(true, false, false, false, true, false).unwrap();
}
#[test]
fn phone_definition_features_realize_in_rules_and_declared_defaults_without_rewriting_identity() {
    let source = occurrence::intent([segment(10)]);
    for use_default in [false, true] {
        let mut setup = Setup::new();
        definition_features(&mut setup, features("opaque cue 7", boolean(true)));
        if use_default {
            defaults_only(&mut setup);
        }
        let profile = profile(&setup);
        let defaults = default(10);
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
        assert_eq!(receipt.definition().identity(), &native::id("phone/t"));
        assert!(core::ptr::eq(
            receipt.aspiration().unwrap().feature().unwrap(),
            &setup.inventory.phones().as_slice()[0]
                .features()
                .get()
                .as_slice()[0]
        ));
        let inherited = if use_default {
            receipt
                .default_output_features()
                .unwrap()
                .features()
                .next()
                .unwrap()
        } else {
            receipt.features().unwrap().features().next().unwrap()
        };
        assert_eq!(inherited.layer(), OutputFeatureLayer::PhoneDefinition);
        assert!(matches!(
            prepared.events()[0],
            VoiceEvent::phone(SpeechPhoneInput {
                phone: conduit_speech::EnglishPhone::t_aspirated,
                ..
            })
        ));
        assert_eq!(
            pcm(prepared.renderer().unwrap(), 1),
            pcm(prepared.renderer().unwrap(), 128)
        );
        assert!(prepare_global_intent(
            &source,
            &setup.inventory,
            &setup.voice,
            &setup.boundaries,
            &setup.rules,
            &setup.policy,
            &[setup.evidence(&defaults)]
        )
        .is_err());
    }
}
#[test]
fn occurrence_default_overrides_definition_only_in_declared_default_branch() {
    let source = occurrence::intent([segment(10)]);
    for use_default in [false, true] {
        let mut setup = Setup::new();
        definition_features(&mut setup, features("opaque cue 7", boolean(true)));
        if use_default {
            defaults_only(&mut setup);
        }
        let profile = profile(&setup);
        let defaults = observation(10, features("opaque cue 7", boolean(false)));
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
        let expected = if use_default {
            conduit_speech::EnglishPhone::t
        } else {
            conduit_speech::EnglishPhone::t_aspirated
        };
        assert!(
            matches!(prepared.events()[0], VoiceEvent::phone(SpeechPhoneInput {phone, ..}) if phone == expected)
        );
        if use_default {
            let receipt = &prepared.phones()[0];
            assert_eq!(
                receipt
                    .default_output_features()
                    .unwrap()
                    .features()
                    .next()
                    .unwrap()
                    .layer(),
                OutputFeatureLayer::DefaultRealization
            );
            assert!(core::ptr::eq(
                receipt.aspiration().unwrap().feature().unwrap(),
                &defaults.features().get().as_slice()[0]
            ));
        }
    }
}
#[test]
fn unknown_occurrence_default_replaces_known_definition_and_refuses_at_late_event() {
    let mut setup = Setup::new();
    definition_features(&mut setup, features("opaque cue 7", boolean(true)));
    defaults_only(&mut setup);
    let profile = profile(&setup);
    let source = occurrence::intent([segment(10), segment(11)]);
    let first = default(10);
    let second = observation(
        11,
        features("opaque cue 7", FeatureSpecification::unknown()),
    );
    assert!(
        matches!(prepare_aspirated_global_intent(&source, &setup.inventory, &profile, &setup.boundaries, &setup.rules, &setup.policy, &[setup.evidence(&first), setup.evidence(&second)]), Err(GlobalIntentRefusal::DefaultProfile {event: 1, reason: GlobalDefaultProfileRefusal::AcousticFeature(FeatureRealizationRefusal::Unresolved(value))}) if core::ptr::eq(value, second.features().get().as_slice()[0].specification()))
    );
}

#[test]
fn sourced_feature_realization_retains_exact_coverage_and_refuses_stale_material_before_choices() {
    use conduit_speech::{
        intent_sources::IntentSourceMaterial,
        sourced_global_intent::{prepare_sourced_aspirated_global_intent, SourcedGlobalRefusal},
    };
    let mut setup = Setup::new();
    definition_features(&mut setup, features("opaque cue 7", boolean(true)));
    defaults_only(&mut setup);
    let profile = profile(&setup);
    let source = occurrence::intent([segment(10), segment(11)]);
    let first = observation(10, features("opaque cue 7", boolean(false)));
    let second = default(11);
    let text = LanguageText::new(
        LanguageTextId::new("source".into()).unwrap(),
        source.language().clone(),
        LanguageTextRevisionId::new("source revision".into()).unwrap(),
        "t".into(),
    )
    .unwrap();
    let materials = [IntentSourceMaterial::Text(&text); 2];
    let evidence = [setup.evidence(&first), setup.evidence(&second)];
    let prepared = prepare_sourced_aspirated_global_intent(
        &source,
        &materials,
        &setup.inventory,
        &profile,
        &setup.boundaries,
        &setup.rules,
        &setup.policy,
        &evidence,
    )
    .unwrap();
    assert!(core::ptr::eq(prepared.source(), &source));
    assert!(core::ptr::eq(prepared.realization().source(), &source));
    assert_eq!(prepared.sources().receipts().len(), 2);
    assert_eq!(
        prepared.realization().phones()[0]
            .aspiration()
            .unwrap()
            .feature()
            .unwrap(),
        &first.features().get().as_slice()[0]
    );
    let direct = prepare_aspirated_global_intent(
        &source,
        &setup.inventory,
        &profile,
        &setup.boundaries,
        &setup.rules,
        &setup.policy,
        &evidence,
    )
    .unwrap();
    assert_eq!(
        pcm(prepared.renderer().unwrap(), 1),
        pcm(direct.renderer().unwrap(), 128)
    );
    let stale = LanguageText::new(
        text.identity().clone(),
        text.language().clone(),
        LanguageTextRevisionId::new("stale revision".into()).unwrap(),
        "t".into(),
    )
    .unwrap();
    assert!(matches!(
        prepare_sourced_aspirated_global_intent(
            &source,
            &[
                IntentSourceMaterial::Text(&text),
                IntentSourceMaterial::Text(&stale)
            ],
            &setup.inventory,
            &profile,
            &setup.boundaries,
            &setup.rules,
            &setup.policy,
            &[]
        ),
        Err(SourcedGlobalRefusal::Sources(_))
    ));
}
