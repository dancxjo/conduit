//! Synthetic compatibility evidence for the same original multiword carrier.
//! This helper invokes production lexical entrances and their explicit acoustic
//! backend refusal. It neither trains a checkpoint nor invents a selected Back.
#![allow(dead_code)]
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_speech::{
    carrier_identity::{canonical_carrier_bytes, checked_carrier_identity},
    correspondence::PreparedSpeechCorrespondence,
    model_linguistic_projection::*,
    semantic::*,
};

pub fn vocabulary(
    original: &PreparedSpeechCorrespondence<'_, '_>,
) -> SpeechModelLinguisticVocabulary {
    let definitions = original.inventory().inventory();
    SpeechModelLinguisticVocabulary::new(
        original.inventory().basis().clone(),
        SpeechModelVocabularyId::new("proof/you-would/lexical-v1".into()).unwrap(),
        BoundedSequence::try_from_iter(definitions.phonemes().as_slice().iter().enumerate().map(
            |(index, definition)| {
                SpeechModelPhonemeToken::new(definition.identity().clone(), 100 + index as u32)
                    .unwrap()
            },
        ))
        .unwrap(),
        BoundedSequence::try_from_iter(definitions.phones().as_slice().iter().enumerate().map(
            |(index, definition)| {
                SpeechModelPhoneToken::new(definition.identity().clone(), 200 + index as u32)
                    .unwrap()
            },
        ))
        .unwrap(),
        SpeechModelProjectionVersion::new(1).unwrap(),
    )
    .unwrap()
}
pub fn checkpoint(vocabulary: &SpeechModelLinguisticVocabulary) -> SpeechModelCheckpointProjection {
    SpeechModelCheckpointProjection::new(
        SpeechModelCheckpointId::new("proof/synthetic-metadata-no-trained-weights".into()).unwrap(),
        vocabulary.clone(),
    )
    .unwrap()
}

/// Owned export evidence. Native metadata is compatibility declarations only;
/// no model artifact, neural PCM, selected implementation or training occurred.
pub struct ModelTerminalEvidence {
    pub original_bytes: Vec<u8>,
    pub source_identity: [u8; 32],
    pub vocabulary_bytes: Vec<u8>,
    pub checkpoint_bytes: Vec<u8>,
    pub training_phoneme_count: usize,
    pub training_phone_count: usize,
    pub inference_phoneme_count: usize,
    pub inference_phone_count: usize,
    pub requested_backend: &'static str,
    pub selected_backend: Option<&'static str>,
    pub refusal: ModelBackendRefusal,
    pub terminal_evidence_identity: [u8; 32],
}
pub fn terminal_evidence(
    original: &PreparedSpeechCorrespondence<'_, '_>,
) -> Result<ModelTerminalEvidence, ModelProjectionRefusal> {
    use ModelProjectionRefusal::Native;
    let vocabulary = vocabulary(original);
    let checkpoint = checkpoint(&vocabulary);
    let training = prepare_model_training_input(original, &vocabulary)?;
    let inference =
        prepare_model_inference_input(original, &vocabulary, &checkpoint, checkpoint.checkpoint())?;
    assert!(core::ptr::eq(training.original(), original));
    assert!(core::ptr::eq(inference.projection().original(), original));
    assert_eq!(
        training.phone_tokens(),
        inference.projection().phone_tokens()
    );
    assert_eq!(
        training.phoneme_tokens(),
        inference.projection().phoneme_tokens()
    );
    let source_identity = checked_carrier_identity(original).map_err(Native)?;
    assert_eq!(training.carrier_identity(), source_identity);
    assert_eq!(inference.projection().carrier_identity(), source_identity);
    let counts = (
        training.phoneme_tokens().len(),
        training.phone_tokens().len(),
        inference.projection().phoneme_tokens().len(),
        inference.projection().phone_tokens().len(),
    );
    let refused = inference.refuse_unsupported_backend();
    let original_bytes =
        canonical_carrier_bytes(refused.input.projection().original()).map_err(Native)?;
    let vocabulary_bytes = vocabulary.clone().encode().map_err(Native)?;
    let checkpoint_bytes = checkpoint.clone().encode().map_err(Native)?;
    let requested_backend = "ConduitVoice acoustic realization";
    let mut terminal = Vec::new();
    terminal.extend_from_slice(&source_identity);
    for bytes in [&vocabulary_bytes, &checkpoint_bytes] {
        terminal.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        terminal.extend_from_slice(bytes);
    }
    terminal.extend_from_slice(requested_backend.as_bytes());
    terminal.extend_from_slice(b"NoAdmittedAcousticBack");
    let terminal_evidence_identity =
        conduit_core::semantic_digest("proof/shared-model-terminal@1", &terminal);
    Ok(ModelTerminalEvidence {
        original_bytes,
        source_identity,
        vocabulary_bytes,
        checkpoint_bytes,
        training_phoneme_count: counts.0,
        training_phone_count: counts.1,
        inference_phoneme_count: counts.2,
        inference_phone_count: counts.3,
        requested_backend,
        selected_backend: None,
        refusal: refused.reason,
        terminal_evidence_identity,
    })
}
