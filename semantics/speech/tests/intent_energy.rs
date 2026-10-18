#![cfg(feature = "semantic-bindings")]
#[allow(dead_code)]
#[path = "common/correspondence.rs"]
mod fixture;
use conduit_audio::*;
use conduit_core::Unit;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_speech::{
    correspondence::PreparedSpeechCorrespondence, intent_energy::*,
    ipa_inventory::PreparedIpaInventory, semantic::*,
};

fn anchor(id: u64) -> AudioTrajectoryAnchor {
    AudioTrajectoryAnchor::new(
        AudioOriginIdentity::new(1).unwrap(),
        AudioTimelineIdentity::new(id).unwrap(),
    )
    .unwrap()
}
fn time(n: u64) -> AudioExactTimeOffset {
    AudioExactTimeOffset::new(1, n).unwrap()
}
fn provenance() -> AudioTrajectoryProvenance {
    AudioTrajectoryProvenance::new(
        AudioTrajectoryProvenanceKind::Authored,
        "declared normalized reference".into(),
        None,
    )
    .unwrap()
}
fn basis(id: &str, power: bool) -> AudioDecibelBasis {
    let reference = AudioDecibelReference::new(
        1,
        id.into(),
        1,
        provenance(),
        if power {
            AudioDecibelReferenceRole::Power
        } else {
            AudioDecibelReferenceRole::AmplitudeMagnitude
        },
        Unit::One,
    )
    .unwrap();
    AudioDecibelBasis::new(
        if power {
            AudioDecibelConvention::PowerTenLog10
        } else {
            AudioDecibelConvention::AmplitudeTwentyLog10
        },
        reference,
    )
    .unwrap()
}
fn ratio(n: u64) -> AudioReferencedLevelRatio {
    AudioReferencedLevelRatio::new(
        basis("normalized-amplitude", false),
        AudioLevelRatio::amplitude(1, n).unwrap(),
    )
    .unwrap()
}
fn known_error(value: AudioReferencedLevelRatio) -> SpeechEnergyErrorSpecification {
    SpeechEnergyErrorSpecification::known(value.basis().clone(), value.ratio().clone()).unwrap()
}
fn scope(ordinal: u32, timeline: u64) -> SpeechPhoneAcousticScope {
    SpeechPhoneAcousticScope::new(
        anchor(timeline),
        fixture::alignment(fixture::groups())
            .inventory_basis()
            .clone(),
        fixture::reference("phones", ordinal),
        fixture::source(),
    )
    .unwrap()
}
fn target(scope: SpeechPhoneAcousticScope) -> SpeechEnergyTarget {
    let trajectory = AudioQuantityTrajectory::new(
        scope.anchor().clone(),
        AudioTrajectoryEndpoints::RightContinuousFinalIncluded,
        AudioTrajectoryOutside::Refuse,
        provenance(),
        BoundedSequence::try_from_iter([AudioTrajectorySegment::new(
            time(1),
            AudioTrajectoryInterpolation::Step,
            AudioTrajectoryQuantity::amplitude(1, 1).unwrap(),
            AudioTrajectoryQuantity::amplitude(1, 1).unwrap(),
            time(0),
        )
        .unwrap()])
        .unwrap(),
    )
    .unwrap();
    let control = AudioScopedControlTrajectory::new(
        "energy".into(),
        AudioTrajectoryDomain::Amplitude,
        0,
        "original-phone".into(),
        trajectory,
    )
    .unwrap();
    SpeechEnergyTarget::new(basis("normalized-amplitude", false), control, scope).unwrap()
}
fn observation(
    scope: SpeechPhoneAcousticScope,
    n: u64,
    uncertainty: SpeechEnergyErrorSpecification,
) -> SpeechEnergyObservation {
    observation_as(
        scope,
        n,
        uncertainty,
        SpeechEnergyObservationInterpretation::Measured,
    )
}
fn observation_as(
    scope: SpeechPhoneAcousticScope,
    n: u64,
    uncertainty: SpeechEnergyErrorSpecification,
    interpretation: SpeechEnergyObservationInterpretation,
) -> SpeechEnergyObservation {
    SpeechEnergyObservation::new(
        uncertainty,
        fixture::confidence(),
        time(1),
        interpretation,
        fixture::provenance(),
        scope,
        time(0),
        ratio(n),
    )
    .unwrap()
}
fn binding(
    scope: SpeechPhoneAcousticScope,
    n: u64,
    uncertainty: SpeechEnergyErrorSpecification,
) -> SpeechEnergyBinding {
    SpeechEnergyBinding::new(
        Some(observation(scope.clone(), n, uncertainty)),
        target(scope),
    )
    .unwrap()
}

#[test]
fn original_target_observation_and_conversion_are_separately_retained() {
    let (inventory, profile, text, intent, phonemes, phones) = (
        fixture::inventory(),
        fixture::profile(),
        fixture::text(),
        fixture::intent(),
        fixture::phonemes(),
        fixture::phones(),
    );
    let (phone_binding, phoneme_binding) = fixture::bindings();
    let inventory = PreparedIpaInventory::prepare(
        &inventory,
        &profile,
        profile.variety(),
        profile.revision(),
        &[phone_binding],
        &[phoneme_binding],
    )
    .unwrap();
    let alignment = fixture::alignment(fixture::groups());
    let original = PreparedSpeechCorrespondence::prepare(
        &intent, &phonemes, &phones, &text, &alignment, &inventory,
    )
    .unwrap();
    let target_only = SpeechEnergyBinding::new(None, target(scope(0, 1))).unwrap();
    let prepared_target = prepare_intent_energy(&original, 0, &anchor(1), &target_only).unwrap();
    assert!(prepared_target.observation().is_none());
    assert!(matches!(
        prepared_target.observed_decibels(),
        Err(IntentEnergyRefusal::MissingObservation)
    ));
    let request = AudioReferencedAmplitudePowerRequest::new(
        AudioRelativeAmplitude::new(1, 1).unwrap(),
        AudioAmplitudePowerReferences::new(
            basis("normalized-amplitude", false).reference().clone(),
            basis("normalized-power", true).reference().clone(),
            AudioAmplitudePowerRelationship::SamePositiveProportionality,
        )
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(
        prepared_target.observed_power(&request),
        Err(IntentEnergyRefusal::MissingObservation)
    ));
    for interpretation in [
        SpeechEnergyObservationInterpretation::Measured,
        SpeechEnergyObservationInterpretation::Inferred,
    ] {
        let supplied = observation_as(
            scope(0, 1),
            10,
            SpeechEnergyErrorSpecification::unknown(),
            interpretation,
        );
        let supplied =
            SpeechEnergyObservation::decode(&supplied.clone().encode().unwrap()).unwrap();
        let binding = SpeechEnergyBinding::new(Some(supplied), target(scope(0, 1))).unwrap();
        let admitted = prepare_intent_energy(&original, 0, &anchor(1), &binding).unwrap();
        assert_eq!(
            admitted.observation().unwrap().interpretation(),
            &interpretation
        );
        assert_eq!(admitted.observation().unwrap().value(), &ratio(10));
        assert_eq!(admitted.target(), binding.target());
    }
    let before = intent.clone().encode().unwrap();
    let text_before = text.clone().encode().unwrap();
    let states = [
        known_error(ratio(1)),
        SpeechEnergyErrorSpecification::unknown(),
        SpeechEnergyErrorSpecification::unspecified(),
        SpeechEnergyErrorSpecification::not_applicable(),
        SpeechEnergyErrorSpecification::variable(
            BoundedSequence::try_from_iter([ratio(1), ratio(2)]).unwrap(),
        )
        .unwrap(),
        SpeechEnergyErrorSpecification::gradient(fixture::confidence(), ratio(1)).unwrap(),
    ];
    for state in states {
        let value = binding(scope(0, 1), 10, state.clone());
        let prepared = prepare_intent_energy(&original, 0, &anchor(1), &value).unwrap();
        assert!(core::ptr::eq(prepared.original(), &original));
        assert_eq!(
            prepared.observation().unwrap().absolute_linear_error(),
            &state
        );
        assert_eq!(prepared.target(), value.target());
        assert_eq!(prepared.observation().unwrap().value(), &ratio(10));
        let db = prepared.observed_decibels().unwrap();
        assert_eq!(db.original(), &ratio(10));
        assert!(!db.executions().is_empty());
        let request = AudioReferencedAmplitudePowerRequest::new(
            AudioRelativeAmplitude::new(1, 10).unwrap(),
            AudioAmplitudePowerReferences::new(
                basis("normalized-amplitude", false).reference().clone(),
                basis("normalized-power", true).reference().clone(),
                AudioAmplitudePowerRelationship::SamePositiveProportionality,
            )
            .unwrap(),
        )
        .unwrap();
        let power = prepared.observed_power(&request).unwrap();
        assert_eq!(power.original(), &request);
        assert_eq!(
            power.result().ratio(),
            &AudioLevelRatio::power(1, 100).unwrap()
        );
        assert!(!power.square().source_program_hex().is_empty());
    }
    assert_eq!(intent.clone().encode().unwrap(), before);
    assert_eq!(text.clone().encode().unwrap(), text_before);
}

#[test]
fn custody_uncertainty_and_unsupported_log_profiles_refuse() {
    let (inventory, profile, text, intent, phonemes, phones) = (
        fixture::inventory(),
        fixture::profile(),
        fixture::text(),
        fixture::intent(),
        fixture::phonemes(),
        fixture::phones(),
    );
    let (pb, fb) = fixture::bindings();
    let inventory = PreparedIpaInventory::prepare(
        &inventory,
        &profile,
        profile.variety(),
        profile.revision(),
        &[pb],
        &[fb],
    )
    .unwrap();
    let alignment = fixture::alignment(fixture::groups());
    let original = PreparedSpeechCorrespondence::prepare(
        &intent, &phonemes, &phones, &text, &alignment, &inventory,
    )
    .unwrap();
    let unknown = SpeechEnergyErrorSpecification::unknown();
    let value = binding(scope(0, 1), 3, unknown.clone());
    let prepared = prepare_intent_energy(&original, 0, &anchor(1), &value).unwrap();
    assert!(matches!(
        prepared.observed_decibels(),
        Err(IntentEnergyRefusal::Decibel(
            AudioDecibelRefusal::UnsupportedExactPowerOfTen(_)
        ))
    ));
    assert!(matches!(
        prepare_intent_energy(&original, 7, &anchor(1), &value),
        Err(IntentEnergyRefusal::Event)
    ));
    assert!(matches!(
        prepare_intent_energy(&original, 1, &anchor(1), &value),
        Err(IntentEnergyRefusal::Occurrence)
    ));
    assert!(matches!(
        prepare_intent_energy(&original, 0, &anchor(2), &value),
        Err(IntentEnergyRefusal::Anchor)
    ));
    let owner_scope = scope(0, 1);
    let other_source = LanguageTextSegmentRef::new(
        *fixture::source().kind(),
        fixture::source().language().clone(),
        fixture::source().range().clone(),
        fixture::source().revision_id().clone(),
        LanguageTextId::new("other-text".into()).unwrap(),
    )
    .unwrap();
    let foreign_source = SpeechPhoneAcousticScope::new(
        anchor(1),
        owner_scope.inventory_basis().clone(),
        owner_scope.occurrence().clone(),
        other_source,
    )
    .unwrap();
    let wrong_source = binding(foreign_source, 10, unknown.clone());
    assert!(matches!(
        prepare_intent_energy(&original, 0, &anchor(1), &wrong_source),
        Err(IntentEnergyRefusal::Source)
    ));
    let p = fixture::profile();
    let revision = SpeechSegmentRevisionId::new("inventory/v4".into()).unwrap();
    let other_profile = SpeechIpaNotationProfile::new(
        p.aliases().clone(),
        p.identity().clone(),
        p.inventory_id().clone(),
        p.provenance().clone(),
        revision.clone(),
        p.units().clone(),
        p.variety().clone(),
    )
    .unwrap();
    let other_basis = SpeechIpaInventoryNotationBasis::new(
        owner_scope.inventory_basis().inventory_id().clone(),
        owner_scope.inventory_basis().language().clone(),
        other_profile,
        revision,
        owner_scope.inventory_basis().variety().clone(),
    )
    .unwrap();
    let foreign_basis = SpeechPhoneAcousticScope::new(
        anchor(1),
        other_basis,
        owner_scope.occurrence().clone(),
        fixture::source(),
    )
    .unwrap();
    let wrong_basis = binding(foreign_basis, 10, unknown.clone());
    assert!(matches!(
        prepare_intent_energy(&original, 0, &anchor(1), &wrong_basis),
        Err(IntentEnergyRefusal::InventoryBasis)
    ));
    let valid_target = target(owner_scope.clone());
    let bad_segment = AudioTrajectorySegment::new(
        time(1),
        AudioTrajectoryInterpolation::Step,
        AudioTrajectoryQuantity::frequency(1, 100).unwrap(),
        AudioTrajectoryQuantity::frequency(1, 100).unwrap(),
        time(0),
    )
    .unwrap();
    let bad_trajectory = AudioQuantityTrajectory::new(
        anchor(1),
        AudioTrajectoryEndpoints::RightContinuousFinalIncluded,
        AudioTrajectoryOutside::Refuse,
        provenance(),
        BoundedSequence::try_from_iter([bad_segment]).unwrap(),
    )
    .unwrap();
    let bad_control = AudioScopedControlTrajectory::new(
        "energy".into(),
        AudioTrajectoryDomain::Amplitude,
        0,
        "original-phone".into(),
        bad_trajectory,
    )
    .unwrap();
    let bad_target = SpeechEnergyTarget::new(
        valid_target.basis().clone(),
        bad_control,
        owner_scope.clone(),
    )
    .unwrap();
    let bad_domain = SpeechEnergyBinding::new(
        Some(observation(owner_scope.clone(), 10, unknown.clone())),
        bad_target,
    )
    .unwrap();
    assert!(prepare_intent_energy(&original, 0, &anchor(1), &bad_domain).is_err());
    assert!(SpeechEnergyTarget::new(
        basis("power", true),
        valid_target.control().clone(),
        owner_scope.clone()
    )
    .is_err());
    assert!(SpeechEnergyObservation::new(
        unknown.clone(),
        fixture::confidence(),
        time(0),
        SpeechEnergyObservationInterpretation::Measured,
        fixture::provenance(),
        owner_scope,
        time(1),
        ratio(10)
    )
    .is_err());
    let foreign = AudioReferencedLevelRatio::new(
        basis("other-reference", false),
        AudioLevelRatio::amplitude(1, 1).unwrap(),
    )
    .unwrap();
    let broken = binding(scope(0, 1), 10, known_error(foreign));
    assert!(prepare_intent_energy(&original, 0, &anchor(1), &broken).is_err());
    assert!(SpeechEnergyBinding::new(
        Some(observation(scope(1, 1), 10, unknown.clone())),
        target(scope(0, 1))
    )
    .is_err());
    assert!(AudioDecibelBasis::new(
        AudioDecibelConvention::PowerTenLog10,
        basis("amplitude", false).reference().clone()
    )
    .is_err());
    assert!(AudioDecibelReference::new(
        0,
        "zero-reference".into(),
        1,
        provenance(),
        AudioDecibelReferenceRole::Power,
        Unit::One
    )
    .is_err());
    assert!(AudioReferencedLevelRatio::new(
        basis("power", true),
        AudioLevelRatio::amplitude(1, 1).unwrap()
    )
    .is_err());
}
