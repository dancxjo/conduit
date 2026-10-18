//! Same immutable authored multiword carrier reaches actual formant preparation
//! and both lexical entrances, ending in an explicit unsupported acoustic Back.
#![cfg(all(feature = "semantic-bindings", feature = "kernel"))]
#[path = "common/carried_formant_fixture.rs"]
mod fixture;
#[path = "common/shared_projection_model.rs"]
mod model;
use conduit_audio::*;
use conduit_language::{LanguageText, LanguageTextRevisionId, LanguageVariety};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::{
    carried_formant::FrozenCarriedFormant, carrier_identity::canonical_carrier_bytes,
    correspondence::PreparedSpeechCorrespondence, model_linguistic_projection::*, semantic::*,
};
fn anchor() -> AudioTrajectoryAnchor {
    AudioTrajectoryAnchor::new(
        AudioOriginIdentity::new(1).unwrap(),
        AudioTimelineIdentity::new(1).unwrap(),
    )
    .unwrap()
}
fn rate() -> AudioSampleRateBasis {
    AudioSampleRateBasis::new(anchor(), AudioFrameQuantization::Floor, 8000).unwrap()
}

#[test]
fn identical_original_multiword_bytes_survive_formant_and_unsupported_model_terminal() {
    let f = fixture::Fixture::new();
    let inventory = f.inventory().unwrap();
    let original = f.correspondence(&inventory).unwrap();
    let before = canonical_carrier_bytes(&original).unwrap();
    assert_eq!(original.text().text(), "you would");
    assert_eq!(original.phones().tokens().len(), 5);
    let formant =
        FrozenCarriedFormant::prepare(&original, &f.formant, &f.boundaries, &anchor(), &rate())
            .unwrap();
    let model = model::terminal_evidence(&original).unwrap();
    assert_eq!(formant.original_bytes(), before);
    assert_eq!(model.original_bytes, before);
    assert_eq!(formant.source_identity(), &model.source_identity);
    assert_ne!(
        formant.projection_identity(),
        &model.terminal_evidence_identity
    );
    assert_eq!(model.refusal, ModelBackendRefusal::NoAdmittedAcousticBack);
    assert_eq!(model.selected_backend, None);
    assert_eq!(model.requested_backend, "ConduitVoice acoustic realization");
    assert_eq!(
        (model.training_phoneme_count, model.training_phone_count),
        (5, 5)
    );
    assert_eq!(
        (model.inference_phoneme_count, model.inference_phone_count),
        (5, 5)
    );
    assert_eq!(formant.total_frames(), 4000);
    assert_eq!(formant.sample_projection_receipts().len(), 7);
    let vocabulary = SpeechModelLinguisticVocabulary::decode(&model.vocabulary_bytes).unwrap();
    let checkpoint = SpeechModelCheckpointProjection::decode(&model.checkpoint_bytes).unwrap();
    let reread =
        prepare_model_inference_input(&original, &vocabulary, &checkpoint, checkpoint.checkpoint())
            .unwrap();
    assert_eq!(
        reread.projection().carrier_identity(),
        model.source_identity
    );
    assert_eq!(canonical_carrier_bytes(&original).unwrap(), before);
    assert_eq!(vocabulary.basis().variety(), inventory.basis().variety());
    assert_eq!(vocabulary.basis().revision(), inventory.basis().revision());
    // Model IDs are explicit private features, never substituted into public
    // phoneme IDs, phone IDs, alignment or the immutable original carrier.
    assert!(vocabulary
        .phonemes()
        .as_slice()
        .iter()
        .all(|entry| *entry.token() < 200));
    assert!(vocabulary
        .phones()
        .as_slice()
        .iter()
        .all(|entry| *entry.token() >= 200));
}

fn foreign_basis(
    original: &SpeechIpaInventoryNotationBasis,
    variety: LanguageVariety,
    revision: SpeechSegmentRevisionId,
) -> SpeechIpaInventoryNotationBasis {
    let profile = original.profile();
    let profile = SpeechIpaNotationProfile::new(
        profile.aliases().clone(),
        profile.identity().clone(),
        profile.inventory_id().clone(),
        profile.provenance().clone(),
        revision.clone(),
        profile.units().clone(),
        variety.clone(),
    )
    .unwrap();
    SpeechIpaInventoryNotationBasis::new(
        original.inventory_id().clone(),
        original.language().clone(),
        profile,
        revision,
        variety,
    )
    .unwrap()
}
#[test]
fn stale_source_variety_revision_vocabulary_checkpoint_and_formant_profile_refuse() {
    let f = fixture::Fixture::new();
    let inventory = f.inventory().unwrap();
    let original = f.correspondence(&inventory).unwrap();
    let before = canonical_carrier_bytes(&original).unwrap();
    let vocabulary = model::vocabulary(&original);
    let checkpoint = model::checkpoint(&vocabulary);
    let foreign_checkpoint = SpeechModelCheckpointId::new("foreign/checkpoint".into()).unwrap();
    assert!(matches!(
        prepare_model_inference_input(&original, &vocabulary, &checkpoint, &foreign_checkpoint),
        Err(ModelProjectionRefusal::Checkpoint)
    ));
    let varieties = [
        foreign_basis(
            inventory.basis(),
            LanguageVariety::new(
                VarietyId::new("foreign/variety".into()).unwrap(),
                original.intent().language().clone(),
            )
            .unwrap(),
            inventory.basis().revision().clone(),
        ),
        foreign_basis(
            inventory.basis(),
            inventory.basis().variety().clone(),
            SpeechSegmentRevisionId::new("foreign/inventory-revision".into()).unwrap(),
        ),
    ];
    for basis in varieties {
        let swapped = SpeechModelLinguisticVocabulary::new(
            basis,
            vocabulary.identity().clone(),
            vocabulary.phonemes().clone(),
            vocabulary.phones().clone(),
            vocabulary.version().clone(),
        )
        .unwrap();
        assert!(matches!(
            prepare_model_training_input(&original, &swapped),
            Err(ModelProjectionRefusal::InventoryBasis)
        ));
        assert!(prepare_model_inference_input(
            &original,
            &swapped,
            &checkpoint,
            checkpoint.checkpoint()
        )
        .is_err());
    }
    let mut changed = vocabulary.phones().as_slice().to_vec();
    changed[0] = SpeechModelPhoneToken::new(changed[0].phone().clone(), 999).unwrap();
    let swapped = SpeechModelLinguisticVocabulary::new(
        vocabulary.basis().clone(),
        vocabulary.identity().clone(),
        vocabulary.phonemes().clone(),
        fixture::seq(changed),
        vocabulary.version().clone(),
    )
    .unwrap();
    assert!(prepare_model_inference_input(
        &original,
        &swapped,
        &checkpoint,
        checkpoint.checkpoint()
    )
    .is_err());
    let missing = SpeechModelLinguisticVocabulary::new(
        vocabulary.basis().clone(),
        vocabulary.identity().clone(),
        vocabulary.phonemes().clone(),
        fixture::seq(vocabulary.phones().as_slice()[1..].iter().cloned()),
        vocabulary.version().clone(),
    )
    .unwrap();
    assert!(matches!(
        prepare_model_training_input(&original, &missing),
        Err(ModelProjectionRefusal::MissingPhone)
    ));
    let text = original.text();
    let stale = LanguageText::new(
        text.identity().clone(),
        text.language().clone(),
        LanguageTextRevisionId::new("foreign/source-revision".into()).unwrap(),
        text.text().clone(),
    )
    .unwrap();
    assert!(PreparedSpeechCorrespondence::prepare(
        &f.intent,
        &f.phonemes,
        &f.phones,
        &stale,
        &f.alignment,
        &inventory
    )
    .is_err());
    let foreign = SpeechFormantVoiceProfile::new(
        f.formant.identity().clone(),
        SpeechInventoryId::new("foreign/inventory".into()).unwrap(),
        f.formant.language().clone(),
        f.formant.phones().clone(),
    )
    .unwrap();
    assert!(
        FrozenCarriedFormant::prepare(&original, &foreign, &f.boundaries, &anchor(), &rate())
            .is_err()
    );
    assert_eq!(canonical_carrier_bytes(&original).unwrap(), before);
}
