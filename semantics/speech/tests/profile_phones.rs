#![cfg(feature = "semantic-bindings")]
use conduit_core::IeeeF32;
use conduit_plot::rust_binding::{BoundedSequence, NativeBindingRefusal};
use conduit_speech::{
    inventory_admission::*, profile_admission::*, reference_admission::*, semantic,
};
use conduit_speech::{
    EnglishPhone, EnglishStress, Renderer, SpeechPhoneInput, VoiceEvent, MAXIMUM_BLOCK_FRAMES,
};
#[path = "common/native_phone.rs"]
mod fixture;
use fixture::*;
use semantic::*;
fn profile(bindings: Vec<SpeechFormantPhoneBinding>, language: &str) -> SpeechFormantVoiceProfile {
    SpeechFormantVoiceProfile::new(
        "voice/fixture".into(),
        SpeechInventoryId::new("inventory".into()).unwrap(),
        LanguageId::new(language.into()).unwrap(),
        BoundedSequence::try_from_iter(bindings).unwrap(),
    )
    .unwrap()
}
fn binding() -> SpeechFormantPhoneBinding {
    SpeechFormantPhoneBinding::new(definition("opaque/t"), semantic::EnglishPhone::T).unwrap()
}
fn pcm(event: VoiceEvent, block: usize) -> Vec<i16> {
    let events = [event];
    let mut renderer = Renderer::prepare(&events).unwrap();
    let mut samples = Vec::new();
    let mut buffer = [0; MAXIMUM_BLOCK_FRAMES];
    while !renderer.is_complete() {
        let n = renderer.render(&mut buffer[..block]).unwrap();
        samples.extend_from_slice(&buffer[..n]);
    }
    samples
}
#[test]
fn rich_definition_renders_without_fabricating_a_source_phoneme() {
    let (reference, snapshot) = material(PhoneSpecification::known(id("opaque/t")).unwrap());
    let material = resolve_phone(&reference, &snapshot).unwrap();
    let inventory = inventory("inventory", "en", vec![definition("opaque/t")]);
    let resolved = resolve_inventory_phone(&material, &inventory).unwrap();
    let profile = profile(vec![binding()], "en");
    let stress = StressSpecification::known(SpeechStress::Primary).unwrap();
    let prepared = prepare_profile_phone(&resolved, &profile, &stress).unwrap();
    assert!(core::ptr::eq(prepared.source(), &resolved));
    assert!(core::ptr::eq(prepared.profile(), &profile));
    assert!(core::ptr::eq(
        prepared.binding(),
        &profile.phones().as_slice()[0]
    ));
    assert!(core::ptr::eq(prepared.stress(), &stress));
    assert_eq!(prepared.compiled_source_id(), conduit_speech::SOURCE_ID);
    assert_eq!(prepared.event().realization(), None);
    let expected = VoiceEvent::phone(SpeechPhoneInput {
        phone: EnglishPhone::t,
        stress: EnglishStress::primary,
    });
    assert_eq!(pcm(prepared.event(), 1), pcm(expected, 128));
}
#[test]
fn native_binding_refuses_a_conflicting_ipa_model() {
    assert!(matches!(
        SpeechFormantPhoneBinding::new(definition("opaque/t"), semantic::EnglishPhone::Iy),
        Err(NativeBindingRefusal::ViolatedInvariant { .. })
    ));
}

#[test]
fn affricate_projection_requires_explicit_single_phone_tie_bar() {
    for (terminal, ipa, cluster) in [
        (semantic::EnglishPhone::Ch, "t͡ʃ", "tʃ"),
        (semantic::EnglishPhone::Jh, "d͡ʒ", "dʒ"),
    ] {
        let definition = |spelling: &str| {
            SpeechPhone::new(
                BoundedSequence::new(),
                SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
                id("phone/affricate"),
                spelling.into(),
                SpeechSegmentStatus::Core,
            )
            .unwrap()
        };
        let checked = SpeechFormantPhoneBinding::new(definition(ipa), terminal).unwrap();
        assert_eq!(checked.definition().ipa(), ipa);
        assert!(SpeechFormantPhoneBinding::new(definition(cluster), terminal).is_err());
    }
}
#[test]
fn unsupported_and_ambiguous_profile_bindings_are_distinct() {
    let (reference, snapshot) = material(PhoneSpecification::known(id("opaque/t")).unwrap());
    let material = resolve_phone(&reference, &snapshot).unwrap();
    let inventory = inventory("inventory", "en", vec![definition("opaque/t")]);
    let resolved = resolve_inventory_phone(&material, &inventory).unwrap();
    let stress = StressSpecification::unknown();
    let unsupported = profile(vec![], "en");
    assert!(matches!(
        prepare_profile_phone(&resolved, &unsupported, &stress),
        Err(ProfileRefusal::UnsupportedPhone)
    ));
    let ambiguous = profile(vec![binding(), binding()], "en");
    assert!(matches!(
        prepare_profile_phone(&resolved, &ambiguous, &stress),
        Err(ProfileRefusal::AmbiguousBinding)
    ));
    let foreign = profile(vec![binding()], "es");
    assert!(matches!(
        prepare_profile_phone(&resolved, &foreign, &stress),
        Err(ProfileRefusal::Basis(_))
    ));
}
#[test]
fn mismatching_definition_metadata_refuses_even_with_the_same_id_and_ipa() {
    let (reference, snapshot) = material(PhoneSpecification::known(id("opaque/t")).unwrap());
    let material = resolve_phone(&reference, &snapshot).unwrap();
    let inventory = inventory("inventory", "en", vec![definition("opaque/t")]);
    let resolved = resolve_inventory_phone(&material, &inventory).unwrap();
    let different = SpeechPhone::new(
        BoundedSequence::new(),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        id("opaque/t"),
        "t".into(),
        SpeechSegmentStatus::Core,
    )
    .unwrap();
    let profile = profile(
        vec![SpeechFormantPhoneBinding::new(different, semantic::EnglishPhone::T).unwrap()],
        "en",
    );
    assert!(matches!(
        prepare_profile_phone(&resolved, &profile, &StressSpecification::unknown()),
        Err(ProfileRefusal::DefinitionSnapshot)
    ));
}
#[test]
fn stress_states_are_preserved_or_returned_unresolved() {
    let (reference, snapshot) = material(PhoneSpecification::known(id("opaque/t")).unwrap());
    let material = resolve_phone(&reference, &snapshot).unwrap();
    let inventory = inventory("inventory", "en", vec![definition("opaque/t")]);
    let resolved = resolve_inventory_phone(&material, &inventory).unwrap();
    let profile = profile(vec![binding()], "en");
    for (stress, expected) in [
        (StressSpecification::unknown(), EnglishStress::unknown),
        (
            StressSpecification::unspecified(),
            EnglishStress::unspecified,
        ),
    ] {
        let prepared = prepare_profile_phone(&resolved, &profile, &stress).unwrap();
        assert_eq!(prepared.stress(), &stress);
        assert_eq!(
            prepared.event(),
            VoiceEvent::phone(SpeechPhoneInput {
                phone: EnglishPhone::t,
                stress: expected
            })
        );
    }
    for stress in [
        StressSpecification::not_applicable(),
        StressSpecification::variable(
            BoundedSequence::try_from_iter([SpeechStress::Primary]).unwrap(),
        )
        .unwrap(),
        StressSpecification::gradient(
            SpeechConfidence::new(IeeeF32::from_value(0.25)).unwrap(),
            SpeechStress::Primary,
        )
        .unwrap(),
    ] {
        match prepare_profile_phone(&resolved, &profile, &stress) {
            Err(ProfileRefusal::UnsupportedStress(actual)) => assert_eq!(actual, stress),
            _ => panic!("must preserve unresolved stress"),
        }
    }
}

fn feature() -> SpeechFeatureBundle {
    SpeechFeatureBundle::new(
        BoundedSequence::try_from_iter([SpeechFeature::new(
            SpeechFeatureId::new("voice".into()).unwrap(),
            FeatureSpecification::unknown(),
        )
        .unwrap()])
        .unwrap(),
    )
    .unwrap()
}
#[test]
fn supplied_feature_constraints_are_never_silently_discarded() {
    let definition = SpeechPhone::new(
        BoundedSequence::new(),
        feature(),
        id("opaque/t"),
        "t".into(),
        SpeechSegmentStatus::Allophonic,
    )
    .unwrap();
    let (reference, snapshot) = material(PhoneSpecification::known(id("opaque/t")).unwrap());
    let material = resolve_phone(&reference, &snapshot).unwrap();
    let inventory = inventory("inventory", "en", vec![definition.clone()]);
    let resolved = resolve_inventory_phone(&material, &inventory).unwrap();
    let profile = profile(
        vec![SpeechFormantPhoneBinding::new(definition, semantic::EnglishPhone::T).unwrap()],
        "en",
    );
    assert!(matches!(
        prepare_profile_phone(&resolved, &profile, &StressSpecification::unknown()),
        Err(ProfileRefusal::UnsupportedDefinitionFeatures)
    ));
}
#[test]
fn observed_token_feature_constraints_refuse_separately() {
    let (reference, snapshot) = material_with_features(
        PhoneSpecification::known(id("opaque/t")).unwrap(),
        feature(),
    );
    let material = resolve_phone(&reference, &snapshot).unwrap();
    let inventory = inventory("inventory", "en", vec![definition("opaque/t")]);
    let resolved = resolve_inventory_phone(&material, &inventory).unwrap();
    let profile = profile(vec![binding()], "en");
    assert!(matches!(
        prepare_profile_phone(&resolved, &profile, &StressSpecification::unknown()),
        Err(ProfileRefusal::UnsupportedTokenFeatures)
    ));
}
