#![cfg(feature = "semantic-bindings")]
#[path = "common/carried_formant_fixture.rs"]
mod fixture;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_speech::{
    complete_carrier::{CompleteCarrierAcoustics, PreparedCompleteSpeechCarrier},
    model_linguistic_projection::*,
    semantic::*,
};

fn make_vocabulary(
    f: &fixture::Fixture,
    basis: &SpeechIpaInventoryNotationBasis,
    token: u32,
) -> SpeechModelLinguisticVocabulary {
    SpeechModelLinguisticVocabulary::new(
        basis.clone(),
        SpeechModelVocabularyId::new("test/lexical-v1".into()).unwrap(),
        BoundedSequence::try_from_iter(f.definitions.phonemes().as_slice().iter().enumerate().map(
            |(index, definition)| {
                SpeechModelPhonemeToken::new(definition.identity().clone(), 100 + index as u32)
                    .unwrap()
            },
        ))
        .unwrap(),
        BoundedSequence::try_from_iter(f.definitions.phones().as_slice().iter().enumerate().map(
            |(index, definition)| {
                SpeechModelPhoneToken::new(definition.identity().clone(), token + index as u32)
                    .unwrap()
            },
        ))
        .unwrap(),
        SpeechModelProjectionVersion::new(1).unwrap(),
    )
    .unwrap()
}

#[test]
fn shared_lexical_map_preserves_distinct_streams_and_original_through_backend_refusal() {
    let f = fixture::Fixture::new();
    let inventory = f.inventory().unwrap();
    let correspondence = f.correspondence(&inventory).unwrap();
    let structure = f.structure();
    let evidence = f.complete_evidence();
    let original = PreparedCompleteSpeechCarrier::prepare(
        &correspondence,
        &structure,
        &evidence.lexical,
        &f.word_phonemes,
        &evidence.request,
        &evidence.result,
        CompleteCarrierAcoustics::not_supplied(),
    )
    .unwrap();
    let original_bytes = original.canonical_bytes().to_vec();
    let vocabulary = make_vocabulary(&f, inventory.basis(), 17);
    let checkpoint = SpeechModelCheckpointProjection::new(
        SpeechModelCheckpointId::new("synthetic/compatibility-only".into()).unwrap(),
        vocabulary.clone(),
    )
    .unwrap();
    let decoded =
        SpeechModelCheckpointProjection::decode(&checkpoint.clone().encode().unwrap()).unwrap();
    let training = prepare_model_training_input(&original, &vocabulary).unwrap();
    let inference =
        prepare_model_inference_input(&original, &vocabulary, &decoded, decoded.checkpoint())
            .unwrap();
    assert!(core::ptr::eq(training.original(), &original));
    assert!(core::ptr::eq(inference.projection().original(), &original));
    assert_eq!(
        training.phone_tokens(),
        inference.projection().phone_tokens()
    );
    assert_eq!(
        training.phoneme_tokens(),
        inference.projection().phoneme_tokens()
    );
    assert_eq!(training.phone_tokens().len(), f.phones.tokens().len());
    assert_eq!(training.phoneme_tokens().len(), f.phonemes.tokens().len());
    assert_eq!(
        training
            .phone_tokens()
            .iter()
            .map(|token| token.backend_value())
            .collect::<Vec<_>>(),
        (17..22).collect::<Vec<_>>()
    );
    assert_eq!(
        training
            .phoneme_tokens()
            .iter()
            .map(|token| token.backend_value())
            .collect::<Vec<_>>(),
        (100..105).collect::<Vec<_>>()
    );
    assert!(core::ptr::eq(
        training.original().correspondence().alignment(),
        &f.alignment
    ));
    assert!(core::ptr::eq(
        training.original().correspondence().inventory(),
        &inventory
    ));
    assert!(core::ptr::eq(training.original().structure(), &structure));
    assert_eq!(training.carrier_identity(), original.identity());
    assert_eq!(
        inference.projection().carrier_identity(),
        original.identity()
    );
    let refusal = inference.refuse_unsupported_backend();
    assert_eq!(refusal.reason, ModelBackendRefusal::NoAdmittedAcousticBack);
    assert!(core::ptr::eq(
        refusal
            .input
            .projection()
            .original()
            .correspondence()
            .intent(),
        &f.intent
    ));
    assert_eq!(
        refusal.input.projection().original().canonical_bytes(),
        original_bytes
    );
    // Same label, changed content must not pass checkpoint compatibility.
    let stale = make_vocabulary(&f, inventory.basis(), 18);
    assert!(
        prepare_model_inference_input(&original, &stale, &decoded, decoded.checkpoint()).is_err()
    );
    let foreign_checkpoint = SpeechModelCheckpointId::new("another/checkpoint".into()).unwrap();
    assert!(matches!(
        prepare_model_inference_input(&original, &vocabulary, &decoded, &foreign_checkpoint),
        Err(ModelProjectionRefusal::Checkpoint)
    ));
    // No schema-version upgrade is inferred.
    assert!(SpeechModelProjectionVersion::new(2).is_err());
    // Same IDs under another exact inventory revision are ineligible.
    let stale_basis = SpeechIpaInventoryNotationBasis::new(
        inventory.basis().inventory_id().clone(),
        inventory.basis().language().clone(),
        inventory.basis().profile().clone(),
        SpeechSegmentRevisionId::new("stale/revision".into()).unwrap(),
        inventory.basis().variety().clone(),
    );
    assert!(stale_basis.is_err());
}

#[test]
fn executed_compatibility_checks_complete_content_and_reloaded_values() {
    let f = fixture::Fixture::new();
    let profile = &f.notation_profile;
    let basis = SpeechIpaInventoryNotationBasis::new(
        profile.inventory_id().clone(),
        profile.variety().language().clone(),
        profile.clone(),
        profile.revision().clone(),
        profile.variety().clone(),
    )
    .unwrap();
    let first = make_vocabulary(&f, &basis, 1);
    let same = SpeechModelLinguisticVocabulary::decode(&first.clone().encode().unwrap()).unwrap();
    SpeechModelProjectionCompatibility::new(first.clone(), same).unwrap();
    assert!(
        SpeechModelProjectionCompatibility::new(first, make_vocabulary(&f, &basis, 2)).is_err()
    );
}

#[test]
fn duplicate_and_foreign_vocabulary_entries_refuse_before_terminal_input() {
    let f = fixture::Fixture::new();
    let profile = &f.notation_profile;
    let inventory = f.inventory().unwrap();
    let correspondence = f.correspondence(&inventory).unwrap();
    let structure = f.structure();
    let evidence = f.complete_evidence();
    let original = PreparedCompleteSpeechCarrier::prepare(
        &correspondence,
        &structure,
        &evidence.lexical,
        &f.word_phonemes,
        &evidence.request,
        &evidence.result,
        CompleteCarrierAcoustics::not_supplied(),
    )
    .unwrap();
    let valid = make_vocabulary(&f, inventory.basis(), 17);
    let duplicate = SpeechModelLinguisticVocabulary::new(
        valid.basis().clone(),
        valid.identity().clone(),
        valid.phonemes().clone(),
        BoundedSequence::try_from_iter([valid.phones()[0].clone(), valid.phones()[0].clone()])
            .unwrap(),
        valid.version().clone(),
    )
    .unwrap();
    assert!(matches!(
        prepare_model_training_input(&original, &duplicate),
        Err(ModelProjectionRefusal::DuplicatePhone)
    ));
    let foreign = SpeechModelLinguisticVocabulary::new(
        valid.basis().clone(),
        valid.identity().clone(),
        valid.phonemes().clone(),
        BoundedSequence::try_from_iter([SpeechModelPhoneToken::new(
            PhoneId::new("foreign/phone".into()).unwrap(),
            99,
        )
        .unwrap()])
        .unwrap(),
        valid.version().clone(),
    )
    .unwrap();
    assert!(matches!(
        prepare_model_training_input(&original, &foreign),
        Err(ModelProjectionRefusal::ForeignPhone)
    ));
    // Valid checked basis but another notation-profile identity is a different
    // declared vocabulary, even with unchanged token spellings and inventory ID.
    let alternate_profile = SpeechIpaNotationProfile::new(
        profile.aliases().clone(),
        SpeechIpaNotationProfileId::new("alternate/profile".into()).unwrap(),
        profile.inventory_id().clone(),
        profile.provenance().clone(),
        profile.revision().clone(),
        profile.units().clone(),
        profile.variety().clone(),
    )
    .unwrap();
    let alternate_basis = SpeechIpaInventoryNotationBasis::new(
        alternate_profile.inventory_id().clone(),
        valid.basis().language().clone(),
        alternate_profile.clone(),
        alternate_profile.revision().clone(),
        alternate_profile.variety().clone(),
    )
    .unwrap();
    let swapped = make_vocabulary(&f, &alternate_basis, 17);
    assert!(matches!(
        prepare_model_training_input(&original, &swapped),
        Err(ModelProjectionRefusal::InventoryBasis)
    ));
}
