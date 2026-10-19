#![cfg(all(feature = "semantic-bindings", feature = "kernel"))]
#[path = "common/carried_formant_fixture.rs"]
mod fixture;
use conduit_audio::{AudioExactTimeOffset, AudioTrajectoryAnchor};
use conduit_language::{LanguageVariety, VarietyId};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::{
    linguistic_rate::*, semantic::*, syllable_structure::PreparedSyllableStructure,
};

fn anchor(origin: u64) -> AudioTrajectoryAnchor {
    AudioTrajectoryAnchor::new(
        conduit_audio::AudioOriginIdentity::new(origin).unwrap(),
        conduit_audio::AudioTimelineIdentity::new(1).unwrap(),
    )
    .unwrap()
}
fn target(count: u64, numerator: u64, denominator: u64) -> SpeechLinguisticRateTarget {
    SpeechLinguisticRateTarget::new(
        SpeechLinguisticRateProfile::new(1).unwrap(),
        fixture::evidence(),
        SpeechLinguisticRateScope::new(
            anchor(1),
            fixture::basis("phones"),
            fixture::alignment().inventory_basis().clone(),
            fixture::text_ref(LanguageTextSegmentKind::Utterance, 0, 9),
        )
        .unwrap(),
        AudioExactTimeOffset::new(denominator, numerator).unwrap(),
        count,
    )
    .unwrap()
}
#[test]
fn exact_declared_rate_executes_source_and_retains_unreduced_span_and_original_structure() {
    let f = fixture::Fixture::new();
    let inventory = f.inventory().unwrap();
    let carrier = f.correspondence(&inventory).unwrap();
    let structure = PreparedSyllableStructure::prepare(
        &f.intent,
        &f.phones,
        f.source.material(),
        &f.syllables,
        &f.positioned_boundaries,
    )
    .unwrap();
    // 2 syllables / (2/4 second) -> 8/2 syllables per second; no reduction.
    let original_target = target(2, 2, 4);
    let original_bytes = original_target.clone().encode().unwrap();
    let rate = prepare_linguistic_rate(&carrier, &structure, &anchor(1), &original_target).unwrap();
    assert!(core::ptr::eq(rate.original(), &carrier));
    assert!(core::ptr::eq(rate.structure(), &structure));
    assert_eq!(
        rate.admission().target().scope().inventory_basis(),
        inventory.basis()
    );
    assert_eq!(*rate.result().numerator_syllables(), 8);
    assert_eq!(*rate.result().denominator_seconds(), 2);
    assert_eq!(rate.admission().target(), &original_target);
    assert_eq!(original_target.clone().encode().unwrap(), original_bytes);
    let input = SpeechLinguisticRateAdmission::decode(rate.input_canonical()).unwrap();
    assert_eq!(*input.actual_syllable_count(), 2);
    let program =
        conduit_plot::PortableExpressionProgram::from_canonical_hex(rate.source_program_hex())
            .unwrap();
    assert_eq!(
        program.evaluate(rate.input_canonical()).unwrap(),
        rate.output_canonical()
    );
    let raw = SpeechLinguisticRateRawProjection::decode(rate.output_canonical()).unwrap();
    let admitted =
        SpeechSyllablesPerSecond::new(*raw.denominator_seconds(), *raw.numerator_syllables())
            .unwrap();
    assert_eq!(&admitted, rate.result());
    assert!(SpeechSyllablesPerSecond::new(0, 1).is_err());
    assert!(
        conduit_audio::AudioFrequencyHz::decode(&rate.result().clone().encode().unwrap()).is_err()
    );
    let partial = SpeechLinguisticRateTarget::new(
        original_target.profile().clone(),
        original_target.provenance().clone(),
        SpeechLinguisticRateScope::new(
            anchor(1),
            fixture::basis("phones"),
            fixture::alignment().inventory_basis().clone(),
            fixture::text_ref(LanguageTextSegmentKind::Utterance, 0, 3),
        )
        .unwrap(),
        original_target.span().clone(),
        2,
    )
    .unwrap();
    assert!(matches!(
        prepare_linguistic_rate(&carrier, &structure, &anchor(1), &partial),
        Err(LinguisticRateRefusal::Source)
    ));
    assert!(prepare_linguistic_rate(&carrier, &structure, &anchor(2), &original_target).is_err());
    assert!(prepare_linguistic_rate(&carrier, &structure, &anchor(1), &target(3, 2, 4)).is_err());
}
#[test]
fn numeric_profile_refuses_zero_and_out_of_range_durations_and_false_counts() {
    assert!(SpeechLinguisticRateAdmission::new(3, target(2, 2, 4)).is_err());
    for span in [
        AudioExactTimeOffset::new(1, 0).unwrap(),
        AudioExactTimeOffset::new(1, 4294967296).unwrap(),
        AudioExactTimeOffset::new(4294967296, 1).unwrap(),
    ] {
        let accepted = target(2, 1, 1);
        assert!(SpeechLinguisticRateTarget::new(
            accepted.profile().clone(),
            accepted.provenance().clone(),
            accepted.scope().clone(),
            span,
            *accepted.syllable_count()
        )
        .is_err());
    }
}

#[test]
fn foreign_variety_and_independently_rebuilt_structure_refuse() {
    let f = fixture::Fixture::new();
    let inventory = f.inventory().unwrap();
    let carrier = f.correspondence(&inventory).unwrap();
    let structure = PreparedSyllableStructure::prepare(
        &f.intent,
        &f.phones,
        f.source.material(),
        &f.syllables,
        &f.positioned_boundaries,
    )
    .unwrap();
    let original = target(2, 2, 4);
    let basis = inventory.basis();
    let variety = LanguageVariety::new(
        VarietyId::new("foreign/variety".into()).unwrap(),
        basis.language().clone(),
    )
    .unwrap();
    let profile = basis.profile();
    let profile = SpeechIpaNotationProfile::new(
        profile.aliases().clone(),
        profile.identity().clone(),
        profile.inventory_id().clone(),
        profile.provenance().clone(),
        profile.revision().clone(),
        profile.units().clone(),
        variety.clone(),
    )
    .unwrap();
    let foreign = SpeechIpaInventoryNotationBasis::new(
        basis.inventory_id().clone(),
        basis.language().clone(),
        profile,
        basis.revision().clone(),
        variety,
    )
    .unwrap();
    let scope = SpeechLinguisticRateScope::new(
        anchor(1),
        fixture::basis("phones"),
        foreign,
        original.scope().source().clone(),
    )
    .unwrap();
    let foreign = SpeechLinguisticRateTarget::new(
        original.profile().clone(),
        original.provenance().clone(),
        scope,
        original.span().clone(),
        2,
    )
    .unwrap();
    assert!(matches!(
        prepare_linguistic_rate(&carrier, &structure, &anchor(1), &foreign),
        Err(LinguisticRateRefusal::InventoryBasis)
    ));
    let rebuilt = fixture::Fixture::new();
    let rebuilt_structure = PreparedSyllableStructure::prepare(
        &rebuilt.intent,
        &rebuilt.phones,
        rebuilt.source.material(),
        &rebuilt.syllables,
        &rebuilt.positioned_boundaries,
    )
    .unwrap();
    assert!(matches!(
        prepare_linguistic_rate(&carrier, &rebuilt_structure, &anchor(1), &original),
        Err(LinguisticRateRefusal::ForeignStructure)
    ));
}
