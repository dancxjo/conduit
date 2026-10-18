//! One finite lexical projector shared by ConduitVoice preparation entrances.
//! It borrows the opaque admitted carrier; model features never replace it.
//! Version 1 omits acoustic/structural conditioning, retaining that information
//! solely on the original. A trained acoustic backend is not implemented here.
use crate::{correspondence::PreparedSpeechCorrespondence, semantic::*};
use alloc::vec::Vec;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};

#[derive(Debug)]
pub enum ModelProjectionRefusal {
    Native(NativeBindingRefusal),
    InventoryBasis,
    Checkpoint,
    DuplicatePhoneme,
    DuplicatePhone,
    DuplicatePhonemeToken,
    DuplicatePhoneToken,
    ForeignPhoneme,
    ForeignPhone,
    MissingPhoneme,
    MissingPhone,
    Unresolved,
}

/// Private terminal representation. A token is meaningful only with its exact
/// vocabulary and distinct stream role, never as a public linguistic identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModelToken(u32);
impl ModelToken {
    /// Terminal backend access; callers must retain the projection receipt.
    pub fn backend_value(self) -> u32 {
        self.0
    }
}

/// Prepared lexical input, not a normalized replacement IR or playback grant.
pub struct PreparedModelLinguisticProjection<'a, 'carrier, 'inventory> {
    original: &'a PreparedSpeechCorrespondence<'carrier, 'inventory>,
    vocabulary: &'a SpeechModelLinguisticVocabulary,
    identity: [u8; 32],
    phonemes: Vec<ModelToken>,
    phones: Vec<ModelToken>,
}
impl<'a, 'carrier, 'inventory> PreparedModelLinguisticProjection<'a, 'carrier, 'inventory> {
    pub fn original(&self) -> &'a PreparedSpeechCorrespondence<'carrier, 'inventory> {
        self.original
    }
    pub fn carrier_identity(&self) -> [u8; 32] {
        self.identity
    }
    pub fn vocabulary(&self) -> &'a SpeechModelLinguisticVocabulary {
        self.vocabulary
    }
    pub fn phoneme_tokens(&self) -> &[ModelToken] {
        &self.phonemes
    }
    pub fn phone_tokens(&self) -> &[ModelToken] {
        &self.phones
    }
}

/// The actual reusable projector. Both entrances below call this function.
/// No text lookup, normalization, acoustic conversion, inference or training.
fn project<'a, 'carrier, 'inventory>(
    original: &'a PreparedSpeechCorrespondence<'carrier, 'inventory>,
    vocabulary: &'a SpeechModelLinguisticVocabulary,
) -> Result<PreparedModelLinguisticProjection<'a, 'carrier, 'inventory>, ModelProjectionRefusal> {
    use ModelProjectionRefusal::*;
    // Re-admission executes the authored Type contracts; decoding alone is not
    // accepted as a compatibility or inventory receipt.
    SpeechModelLinguisticVocabulary::decode(&vocabulary.clone().encode().map_err(Native)?)
        .map_err(Native)?;
    if vocabulary.basis() != original.inventory().basis() {
        return Err(InventoryBasis);
    }
    let inventory = original.inventory().inventory();
    for (index, value) in vocabulary.phonemes().as_slice().iter().enumerate() {
        if vocabulary.phonemes().as_slice()[..index]
            .iter()
            .any(|other| other.phoneme() == value.phoneme())
        {
            return Err(DuplicatePhoneme);
        }
        if vocabulary.phonemes().as_slice()[..index]
            .iter()
            .any(|other| other.token() == value.token())
        {
            return Err(DuplicatePhonemeToken);
        }
        if !inventory
            .phonemes()
            .as_slice()
            .iter()
            .any(|definition| definition.identity() == value.phoneme())
        {
            return Err(ForeignPhoneme);
        }
    }
    for (index, value) in vocabulary.phones().as_slice().iter().enumerate() {
        if vocabulary.phones().as_slice()[..index]
            .iter()
            .any(|other| other.phone() == value.phone())
        {
            return Err(DuplicatePhone);
        }
        if vocabulary.phones().as_slice()[..index]
            .iter()
            .any(|other| other.token() == value.token())
        {
            return Err(DuplicatePhoneToken);
        }
        if !inventory
            .phones()
            .as_slice()
            .iter()
            .any(|definition| definition.identity() == value.phone())
        {
            return Err(ForeignPhone);
        }
    }
    let phonemes = original
        .phonemes()
        .tokens()
        .as_slice()
        .iter()
        .map(|occurrence| {
            let PhonemeSpecification::Known(id) = occurrence.phoneme() else {
                return Err(Unresolved);
            };
            vocabulary
                .phonemes()
                .as_slice()
                .iter()
                .find(|entry| entry.phoneme() == id)
                .map(|entry| ModelToken(*entry.token()))
                .ok_or(MissingPhoneme)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let phones = original
        .phones()
        .tokens()
        .as_slice()
        .iter()
        .map(|occurrence| {
            let PhoneSpecification::Known(id) = occurrence.phone() else {
                return Err(Unresolved);
            };
            vocabulary
                .phones()
                .as_slice()
                .iter()
                .find(|entry| entry.phone() == id)
                .map(|entry| ModelToken(*entry.token()))
                .ok_or(MissingPhone)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(PreparedModelLinguisticProjection {
        original,
        vocabulary,
        identity: crate::carrier_identity::checked_carrier_identity(original).map_err(Native)?,
        phonemes,
        phones,
    })
}

/// Training-data preparation only; no checkpoint or learned parameters created.
pub fn prepare_model_training_input<'a, 'carrier, 'inventory>(
    original: &'a PreparedSpeechCorrespondence<'carrier, 'inventory>,
    vocabulary: &'a SpeechModelLinguisticVocabulary,
) -> Result<PreparedModelLinguisticProjection<'a, 'carrier, 'inventory>, ModelProjectionRefusal> {
    project(original, vocabulary)
}

pub struct PreparedModelInferenceInput<'a, 'carrier, 'inventory> {
    projection: PreparedModelLinguisticProjection<'a, 'carrier, 'inventory>,
    checkpoint: &'a SpeechModelCheckpointProjection,
}
impl<'a, 'carrier, 'inventory> PreparedModelInferenceInput<'a, 'carrier, 'inventory> {
    pub fn projection(&self) -> &PreparedModelLinguisticProjection<'a, 'carrier, 'inventory> {
        &self.projection
    }
    pub fn checkpoint(&self) -> &'a SpeechModelCheckpointProjection {
        self.checkpoint
    }
    /// No admitted neural acoustic Back consumes this lexical profile yet.
    /// Refusal retains the full prepared input for machine-readable custody.
    pub fn refuse_unsupported_backend(self) -> UnsupportedModelBackend<'a, 'carrier, 'inventory> {
        UnsupportedModelBackend {
            input: self,
            reason: ModelBackendRefusal::NoAdmittedAcousticBack,
        }
    }
}

pub fn prepare_model_inference_input<'a, 'carrier, 'inventory>(
    original: &'a PreparedSpeechCorrespondence<'carrier, 'inventory>,
    vocabulary: &'a SpeechModelLinguisticVocabulary,
    checkpoint: &'a SpeechModelCheckpointProjection,
    selected_checkpoint: &SpeechModelCheckpointId,
) -> Result<PreparedModelInferenceInput<'a, 'carrier, 'inventory>, ModelProjectionRefusal> {
    if checkpoint.checkpoint() != selected_checkpoint {
        return Err(ModelProjectionRefusal::Checkpoint);
    }
    SpeechModelProjectionCompatibility::new(vocabulary.clone(), checkpoint.vocabulary().clone())
        .map_err(ModelProjectionRefusal::Native)?;
    Ok(PreparedModelInferenceInput {
        projection: project(original, vocabulary)?,
        checkpoint,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelBackendRefusal {
    NoAdmittedAcousticBack,
}
pub struct UnsupportedModelBackend<'a, 'carrier, 'inventory> {
    pub input: PreparedModelInferenceInput<'a, 'carrier, 'inventory>,
    pub reason: ModelBackendRefusal,
}
