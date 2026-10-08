//! Explicit reviewed capability and complete incoming model/profile custody.
//! The caller's prepared model is existing input: its entire retained storage is
//! charged before Session allocations, independently of Native family receipts.
use crate::{parser_model_selection::PreparedParserModelSelection, *};
use alloc::sync::Arc;
use core::mem::{align_of, size_of};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParserSessionCapabilities {
    maximum_tokens: u8,
    beam_width: u8,
    maximum_lexical_candidates: u8,
    feature_lookups: u16,
    score_classes: u16,
}
impl ParserSessionCapabilities {
    pub fn maximum_tokens(self) -> u8 {
        self.maximum_tokens
    }
    pub fn beam_width(self) -> u8 {
        self.beam_width
    }
    pub fn maximum_lexical_candidates(self) -> u8 {
        self.maximum_lexical_candidates
    }
    pub fn feature_lookups(self) -> u16 {
        self.feature_lookups
    }
    pub fn score_classes(self) -> u16 {
        self.score_classes
    }
}
/// A future reviewed Window8 owner carries its own complete model and Source
/// contract. There is deliberately no constructor before actual profile and
/// revision/commitment acceptance; FourSlot custody cannot authorize Window8.
pub struct ReviewedWindow8ParserProfile {
    selection: Arc<PreparedParserModelSelection>,
    capabilities: ParserSessionCapabilities,
}
pub enum ParserSessionProfile {
    PinnedFourSlotV2(Arc<PreparedParserModelSelection>),
    ReviewedWindow8(ReviewedWindow8ParserProfile),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParserSessionProfileStorageReceipt {
    pub complete_model_retained_bytes_bound: usize,
    pub lexical_profile_retained_bytes: usize,
    pub selection_owner_bytes_bound: usize,
    pub combined_existing_input_bytes_bound: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParserSessionProfileRefusal {
    Profile,
    UnsupportedProfile,
    Layout,
    Overflow,
    Pressure,
    Model,
}
pub(crate) struct PreparedParserSessionProfile {
    pub(crate) selection: Arc<PreparedParserModelSelection>,
    pub(crate) capabilities: ParserSessionCapabilities,
    pub(crate) storage: ParserSessionProfileStorageReceipt,
}
fn add(a: usize, b: usize) -> Result<usize, ParserSessionProfileRefusal> {
    a.checked_add(b)
        .ok_or(ParserSessionProfileRefusal::Overflow)
}
fn array<T>(capacity: usize) -> Result<usize, ParserSessionProfileRefusal> {
    capacity
        .checked_mul(size_of::<T>())
        .ok_or(ParserSessionProfileRefusal::Overflow)
}
fn lexical_storage(profile: &LanguageLexicalProfile) -> Result<usize, ParserSessionProfileRefusal> {
    use ParserSessionProfileRefusal as R;
    // Unit lexical POS variants own no allocations. A representation change must
    // receive a fresh explicit accounting implementation rather than undercharge.
    if size_of::<LanguageLexicalPos>() != 1 {
        return Err(R::Layout);
    }
    let mut bytes = add(
        profile.identity().capacity(),
        profile.language().get().capacity(),
    )?;
    bytes = add(
        bytes,
        array::<LanguageLexicalEntry>(profile.entries().allocated_capacity())?,
    )?;
    for entry in profile.entries() {
        bytes = add(bytes, entry.surface().capacity())?;
        bytes = add(
            bytes,
            array::<LanguageLexicalCandidate>(entry.candidates().allocated_capacity())?,
        )?;
        for candidate in entry.candidates() {
            bytes = add(bytes, candidate.lemma().capacity())?;
            bytes = add(
                bytes,
                array::<LinguisticTokenFeature>(candidate.morphology().allocated_capacity())?,
            )?;
            for feature in candidate.morphology() {
                bytes = add(
                    bytes,
                    add(feature.name().capacity(), feature.value().capacity())?,
                )?;
            }
        }
    }
    let (implementation, revision) = match profile.provenance() {
        LinguisticDerivationProvenance::DeterministicRule(v) => (v.implementation(), v.revision()),
        LinguisticDerivationProvenance::Library(v) => (v.implementation(), v.revision()),
        LinguisticDerivationProvenance::Model(v) => (v.implementation(), v.revision()),
    };
    add(bytes, add(implementation.capacity(), revision.capacity())?)
}
impl PreparedParserSessionProfile {
    /// This allocation-free admission moves the complete incoming owner. Model,
    /// signature, resource grant, offer/residency and lexical metadata remain live.
    pub(crate) fn admit(
        profile: ParserSessionProfile,
        maximum_existing_input_bytes: usize,
    ) -> Result<Self, ParserSessionProfileRefusal> {
        use ParserSessionProfileRefusal as R;
        let selection = match profile {
            ParserSessionProfile::PinnedFourSlotV2(selection) => selection,
            ParserSessionProfile::ReviewedWindow8(_) => return Err(R::UnsupportedProfile),
        };
        if selection.declaration().is_some()
            || selection.prepared_categorical().dimensions() != (413, 76, 25)
            || selection.prepared_categorical().maximum_score_magnitude()
                > crate::parser_model_selection::V2_MAXIMUM_SCORE
        {
            return Err(R::Profile);
        }
        let model = selection
            .prepared_categorical()
            .storage_receipt()
            .map_err(|_| R::Model)?
            .retained_heap_bytes_bound;
        let lexical = lexical_storage(selection.expected_lexical_profile())?;
        let owner = add(
            add(
                size_of::<PreparedParserModelSelection>(),
                2 * size_of::<usize>(),
            )?,
            4 * align_of::<PreparedParserModelSelection>(),
        )?;
        let combined = add(add(model, lexical)?, owner)?;
        if combined > maximum_existing_input_bytes {
            return Err(R::Pressure);
        }
        Ok(Self {
            selection,
            capabilities: ParserSessionCapabilities {
                maximum_tokens: 4,
                beam_width: 4,
                maximum_lexical_candidates: 4,
                feature_lookups: 25,
                score_classes: 76,
            },
            storage: ParserSessionProfileStorageReceipt {
                complete_model_retained_bytes_bound: model,
                lexical_profile_retained_bytes: lexical,
                selection_owner_bytes_bound: owner,
                combined_existing_input_bytes_bound: combined,
            },
        })
    }
}

fn evidence_storage(
    value: &LinguisticDerivationProvenance,
) -> Result<usize, ParserSessionProfileRefusal> {
    let (implementation, revision) = match value {
        LinguisticDerivationProvenance::DeterministicRule(v) => (v.implementation(), v.revision()),
        LinguisticDerivationProvenance::Library(v) => (v.implementation(), v.revision()),
        LinguisticDerivationProvenance::Model(v) => (v.implementation(), v.revision()),
    };
    add(implementation.capacity(), revision.capacity())
}
fn occurrence_storage(
    value: &LinguisticTokenIdentity,
) -> Result<usize, ParserSessionProfileRefusal> {
    add(
        value.text_identity().get().capacity(),
        value.text_revision().get().capacity(),
    )
}
/// Actual heap of the opaque reviewed lexical producer's complete original
/// output. Vec storage uses allocated capacity, never semantic sequence maximum.
pub(crate) fn lexical_tape_storage(
    tape: &LanguageLexicalTape,
) -> Result<usize, ParserSessionProfileRefusal> {
    use ParserSessionProfileRefusal as R;
    if size_of::<LanguageLexicalCompleteness>() != 1
        || size_of::<LinguisticTokenCategory>() != 1
        || size_of::<LanguageTextFinality>() != 1
    {
        return Err(R::Layout);
    }
    let mut bytes = lexical_storage(tape.profile())?;
    let source = tape.source();
    let material = source.material();
    for capacity in [
        material.identity().get().capacity(),
        material.revision().get().capacity(),
        material.language().get().capacity(),
        material.text().capacity(),
    ] {
        bytes = add(bytes, capacity)?;
    }
    bytes = add(bytes, evidence_storage(source.provenance())?)?;
    if let Some(prior) = source.prior() {
        bytes = add(bytes, prior.revision().get().capacity())?;
    }
    bytes = add(
        bytes,
        array::<LanguageLexicalToken>(tape.tokens().allocated_capacity())?,
    )?;
    for token in tape.tokens() {
        bytes = add(bytes, occurrence_storage(token.identity())?)?;
        if let Some(prior) = token.prior_occurrence() {
            bytes = add(bytes, occurrence_storage(prior)?)?;
        }
        bytes = add(bytes, token.surface().capacity())?;
        bytes = add(
            bytes,
            array::<LanguageLexicalCandidate>(token.candidates().allocated_capacity())?,
        )?;
        for candidate in token.candidates() {
            bytes = add(bytes, candidate.lemma().capacity())?;
            bytes = add(
                bytes,
                array::<LinguisticTokenFeature>(candidate.morphology().allocated_capacity())?,
            )?;
            for feature in candidate.morphology() {
                bytes = add(
                    bytes,
                    add(feature.name().capacity(), feature.value().capacity())?,
                )?;
            }
        }
    }
    Ok(bytes)
}
