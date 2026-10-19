//! One finite lexical projector shared by ConduitVoice preparation entrances.
//! It borrows the complete admitted carrier, including checked structure,
//! commitment and acoustic custody; model features never replace it.
//! Version 1 omits acoustic/structural conditioning, retaining that information
//! solely on the original. A trained acoustic backend is not implemented here.
use crate::{complete_carrier::PreparedCompleteSpeechCarrier, semantic::*};
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

/// The versioned map supplies lexical tokens only. These retained components
/// are available on original(), but never claimed as learned conditioning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelRetainedOnly {
    TextAndAlignment,
    SyllableAndWordStructure,
    ProsodyAndAcoustics,
    LanguageCommitment,
}
const RETAINED_ONLY: &[ModelRetainedOnly] = &[
    ModelRetainedOnly::TextAndAlignment,
    ModelRetainedOnly::SyllableAndWordStructure,
    ModelRetainedOnly::ProsodyAndAcoustics,
    ModelRetainedOnly::LanguageCommitment,
];

/// Prepared lexical input, not a normalized replacement IR or playback grant.
pub struct PreparedModelLinguisticProjection<'a, 'context, 'carrier, 'inventory> {
    original: &'a PreparedCompleteSpeechCarrier<'context, 'carrier, 'inventory>,
    vocabulary: &'a SpeechModelLinguisticVocabulary,
    identity: [u8; 32],
    phonemes: Vec<ModelToken>,
    phones: Vec<ModelToken>,
}
impl<'a, 'context, 'carrier, 'inventory>
    PreparedModelLinguisticProjection<'a, 'context, 'carrier, 'inventory>
{
    pub fn original(&self) -> &'a PreparedCompleteSpeechCarrier<'context, 'carrier, 'inventory> {
        self.original
    }
    pub fn retained_without_conditioning(&self) -> &'static [ModelRetainedOnly] {
        RETAINED_ONLY
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
fn project<'a, 'context, 'carrier, 'inventory>(
    original: &'a PreparedCompleteSpeechCarrier<'context, 'carrier, 'inventory>,
    vocabulary: &'a SpeechModelLinguisticVocabulary,
) -> Result<
    PreparedModelLinguisticProjection<'a, 'context, 'carrier, 'inventory>,
    ModelProjectionRefusal,
> {
    use ModelProjectionRefusal::*;
    // Re-admission executes the authored Type contracts; decoding alone is not
    // accepted as a compatibility or inventory receipt.
    SpeechModelLinguisticVocabulary::decode(&vocabulary.clone().encode().map_err(Native)?)
        .map_err(Native)?;
    if vocabulary.basis() != original.correspondence().inventory().basis() {
        return Err(InventoryBasis);
    }
    let inventory = original.correspondence().inventory().inventory();
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
        .correspondence()
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
        .correspondence()
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
        identity: original.identity(),
        phonemes,
        phones,
    })
}

/// Training-data preparation only; no checkpoint or learned parameters created.
pub fn prepare_model_training_input<'a, 'context, 'carrier, 'inventory>(
    original: &'a PreparedCompleteSpeechCarrier<'context, 'carrier, 'inventory>,
    vocabulary: &'a SpeechModelLinguisticVocabulary,
) -> Result<
    PreparedModelLinguisticProjection<'a, 'context, 'carrier, 'inventory>,
    ModelProjectionRefusal,
> {
    project(original, vocabulary)
}

pub struct PreparedModelInferenceInput<'a, 'context, 'carrier, 'inventory> {
    projection: PreparedModelLinguisticProjection<'a, 'context, 'carrier, 'inventory>,
    checkpoint: &'a SpeechModelCheckpointProjection,
}
impl<'a, 'context, 'carrier, 'inventory>
    PreparedModelInferenceInput<'a, 'context, 'carrier, 'inventory>
{
    pub fn projection(
        &self,
    ) -> &PreparedModelLinguisticProjection<'a, 'context, 'carrier, 'inventory> {
        &self.projection
    }
    pub fn checkpoint(&self) -> &'a SpeechModelCheckpointProjection {
        self.checkpoint
    }
    /// No admitted neural acoustic Back consumes this lexical profile yet.
    /// Refusal retains the full prepared input for machine-readable custody.
    pub fn refuse_unsupported_backend(
        self,
    ) -> UnsupportedModelBackend<'a, 'context, 'carrier, 'inventory> {
        UnsupportedModelBackend {
            input: self,
            reason: ModelBackendRefusal::NoAdmittedAcousticBack,
        }
    }
}

pub fn prepare_model_inference_input<'a, 'context, 'carrier, 'inventory>(
    original: &'a PreparedCompleteSpeechCarrier<'context, 'carrier, 'inventory>,
    vocabulary: &'a SpeechModelLinguisticVocabulary,
    checkpoint: &'a SpeechModelCheckpointProjection,
    selected_checkpoint: &SpeechModelCheckpointId,
) -> Result<PreparedModelInferenceInput<'a, 'context, 'carrier, 'inventory>, ModelProjectionRefusal>
{
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
pub struct UnsupportedModelBackend<'a, 'context, 'carrier, 'inventory> {
    pub input: PreparedModelInferenceInput<'a, 'context, 'carrier, 'inventory>,
    pub reason: ModelBackendRefusal,
}
