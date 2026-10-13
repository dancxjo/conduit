//! Owned admission and original Source execution for one lexical proposal.
//! Full revision/tape production and model selection are separate capabilities.
use crate::{
    lexical_proposer_resource::{LexicalDictionaryStorageReceipt, PreparedLexicalDictionary},
    LanguageLexicalProposalOrigin, LanguageLexicalProposalQuery, LanguageLexicalProposedTape,
    LanguageLexicalProposerDefinition, LanguageLexicalTokenProposal,
};
use alloc::{boxed::Box, vec::Vec};
use conduit_core::{semantic_digest, validate_canonical_structured_value};
use conduit_plot::rust_binding::{
    PreparedNativeFamily, PreparedNativeFamilyLimits, PreparedNativeFamilyStorageReceipt,
    PreparedNativeRustBinding,
};
use conduit_plot::{
    PortableExpressionProgram, PreparedExpressionStorageReceipt,
    PreparedPortableExpressionEvaluator,
};
use core::mem::size_of;

#[derive(Clone, Copy, Debug)]
pub struct LexicalProposerPortLimits {
    pub maximum_frame_bytes: usize,
    pub maximum_retained_bytes: usize,
    pub maximum_preparation_peak_bytes: usize,
    pub maximum_admission_peak_bytes: usize,
    pub maximum_response_requested_bytes: usize,
    pub native: PreparedNativeFamilyLimits,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LexicalProposerPortRefusal {
    Capacity,
    Frame,
    Definition,
    Resource,
    Native,
    Source,
}
#[derive(Clone, Copy, Debug)]
pub struct LexicalProposerPortReceipt {
    pub retained_heap_bytes_bound: usize,
    pub preparation_peak_heap_bytes_bound: usize,
    pub admission_peak_heap_bytes_bound: usize,
    pub response_requested_bytes_bound: usize,
    pub native: PreparedNativeFamilyStorageReceipt,
    pub source: PreparedExpressionStorageReceipt,
    pub dictionary: LexicalDictionaryStorageReceipt,
}
pub struct PreparedLexicalProposerPort {
    dictionary: PreparedLexicalDictionary,
    definition: Box<[u8]>,
    definition_identity: [u8; 32],
    source_identity: [u8; 32],
    source: PreparedPortableExpressionEvaluator,
    family: PreparedNativeFamily,
    maximum_frame_bytes: usize,
    receipt: LexicalProposerPortReceipt,
}
/// Original query, Source output and unknown/reviewed origin remain immutable.
/// Each retained response must be charged by the caller; the port keeps none.
pub struct AdmittedLexicalTokenProposal {
    proposal: LanguageLexicalTokenProposal,
    query: Box<[u8]>,
    output: Box<[u8]>,
    definition_identity: [u8; 32],
    source_identity: [u8; 32],
    dictionary_identity: [u8; 32],
    requested_bytes_bound: usize,
}
/// Full canonical custody after transient generated Native conversion is released.
/// Construction is available only from an admitted original Source response.
pub struct CanonicalLexicalTokenProposal {
    query: Box<[u8]>,
    output: Box<[u8]>,
    definition_identity: [u8; 32],
    source_identity: [u8; 32],
    dictionary_identity: [u8; 32],
}
impl CanonicalLexicalTokenProposal {
    pub fn canonical_query(&self) -> &[u8] {
        &self.query
    }
    pub fn canonical_output(&self) -> &[u8] {
        &self.output
    }
    pub const fn definition_identity(&self) -> [u8; 32] {
        self.definition_identity
    }
    pub const fn source_identity(&self) -> [u8; 32] {
        self.source_identity
    }
    pub const fn dictionary_identity(&self) -> [u8; 32] {
        self.dictionary_identity
    }
    /// Boxed slice payloads have exactly their encoded lengths. This includes
    /// the inline receipt so an owner can conservatively charge a receipt slot.
    pub fn retained_requested_bytes_bound(&self) -> usize {
        size_of::<Self>() + self.query.len() + self.output.len()
    }
}
impl AdmittedLexicalTokenProposal {
    /// Move complete original frames without allocation. The transient typed
    /// conversion is dropped; historical readmission requires a new conversion
    /// reservation and the same exact ready family/Source owner.
    pub fn into_canonical_receipt(self) -> CanonicalLexicalTokenProposal {
        CanonicalLexicalTokenProposal {
            query: self.query,
            output: self.output,
            definition_identity: self.definition_identity,
            source_identity: self.source_identity,
            dictionary_identity: self.dictionary_identity,
        }
    }
    pub fn proposal(&self) -> &LanguageLexicalTokenProposal {
        &self.proposal
    }
    pub fn canonical_query(&self) -> &[u8] {
        &self.query
    }
    pub fn canonical_output(&self) -> &[u8] {
        &self.output
    }
    pub const fn definition_identity(&self) -> [u8; 32] {
        self.definition_identity
    }
    pub const fn source_identity(&self) -> [u8; 32] {
        self.source_identity
    }
    pub const fn dictionary_identity(&self) -> [u8; 32] {
        self.dictionary_identity
    }
    pub const fn requested_bytes_bound(&self) -> usize {
        self.requested_bytes_bound
    }
}
fn add(a: usize, b: usize) -> Result<usize, LexicalProposerPortRefusal> {
    a.checked_add(b).ok_or(LexicalProposerPortRefusal::Capacity)
}
fn mul(a: usize, b: usize) -> Result<usize, LexicalProposerPortRefusal> {
    a.checked_mul(b).ok_or(LexicalProposerPortRefusal::Capacity)
}
fn admit(n: usize, max: usize) -> Result<(), LexicalProposerPortRefusal> {
    if n > max {
        Err(LexicalProposerPortRefusal::Capacity)
    } else {
        Ok(())
    }
}
fn digest_matches(text: &str, digest: [u8; 32]) -> bool {
    let b = text.as_bytes();
    if b.len() != 64 {
        return false;
    }
    let hex = b"0123456789abcdef";
    digest.iter().enumerate().all(|(i, v)| {
        b[2 * i] == hex[usize::from(v >> 4)] && b[2 * i + 1] == hex[usize::from(v & 15)]
    })
}
impl PreparedLexicalProposerPort {
    pub fn prepare(
        dictionary: PreparedLexicalDictionary,
        definition: &[u8],
        limits: LexicalProposerPortLimits,
    ) -> Result<Self, LexicalProposerPortRefusal> {
        use LexicalProposerPortRefusal as R;
        if limits.maximum_frame_bytes == 0
            || limits.maximum_frame_bytes > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
        {
            return Err(R::Capacity);
        }
        admit(definition.len(), limits.maximum_frame_bytes)?;
        let dict = dictionary.storage_receipt();
        let fixed = add(size_of::<Self>(), definition.len())?;
        let base = add(dict.retained_heap_bytes_bound, fixed)?;
        admit(base, limits.maximum_retained_bytes)?;
        admit(base, limits.maximum_preparation_peak_bytes)?;
        let mut native_limits = limits.native;
        native_limits.maximum_input_bytes = native_limits
            .maximum_input_bytes
            .min(limits.maximum_frame_bytes);
        native_limits.maximum_retained_bytes = native_limits
            .maximum_retained_bytes
            .min(limits.maximum_retained_bytes - base);
        native_limits.maximum_preparation_peak_bytes = native_limits
            .maximum_preparation_peak_bytes
            .min(limits.maximum_preparation_peak_bytes - base);
        let mut family = PreparedNativeFamily::prepare(
            &[
                LanguageLexicalTokenProposal::PREPARED_DESCRIPTOR,
                LanguageLexicalProposedTape::PREPARED_DESCRIPTOR,
            ],
            native_limits,
        )
        .map_err(|_| R::Native)?;
        let native = family.storage_receipt();
        let base = add(base, native.retained_heap_bytes_bound)?;
        let mut peak = add(
            add(dict.retained_heap_bytes_bound, fixed)?,
            native.preparation_peak_heap_bytes_bound,
        )?;
        let definition_peak = add(base, native.conversion_requested_bytes_bound)?;
        admit(definition_peak, limits.maximum_preparation_peak_bytes)?;
        peak = peak.max(definition_peak);
        let admitted: LanguageLexicalProposerDefinition =
            family.decode(definition).map_err(|_| R::Native)?;
        if !digest_matches(admitted.dictionary_identity(), dictionary.identity())
            || admitted.language().get() != dictionary.language()
        {
            return Err(R::Definition);
        }
        drop(admitted);
        let definition_identity =
            semantic_digest("language/lexical-proposer-definition@1", definition);
        let definition: Box<[u8]> = definition.into();
        let hex = include_str!(concat!(env!("OUT_DIR"), "/lexical_token_proposal.hex"));
        if !hex.len().is_multiple_of(2) {
            return Err(R::Source);
        }
        let raw_length = hex.len() / 2;
        admit(
            add(base, raw_length)?,
            limits.maximum_preparation_peak_bytes,
        )?;
        let mut raw = Vec::with_capacity(raw_length);
        for i in (0..hex.len()).step_by(2) {
            raw.push(u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| R::Source)?);
        }
        let raw_storage = raw.capacity();
        let decode = PortableExpressionProgram::canonical_decode_storage_bound(&raw)
            .map_err(|_| R::Source)?;
        let decoding_peak = add(add(base, raw_storage)?, decode)?;
        admit(decoding_peak, limits.maximum_preparation_peak_bytes)?;
        peak = peak.max(decoding_peak);
        let program =
            PortableExpressionProgram::from_canonical_bytes_with_storage_limit(&raw, decode)
                .map_err(|_| R::Source)?;
        // Type comparison encoding is a separate admitted phase; the original exact
        // generated input/output schemas are never nominally recast.
        let comparisons = mul(
            add(
                program
                    .input_type
                    .canonical_byte_length()
                    .map_err(|_| R::Source)?
                    .max(8),
                program
                    .output_type
                    .canonical_byte_length()
                    .map_err(|_| R::Source)?
                    .max(8),
            )?,
            4,
        )?;
        let compare_peak = add(decoding_peak, comparisons)?;
        admit(compare_peak, limits.maximum_preparation_peak_bytes)?;
        peak = peak.max(compare_peak);
        if program
            .input_type
            .canonical_bytes()
            .map_err(|_| R::Source)?
            .as_slice()
            != LanguageLexicalProposalQuery::PREPARED_DESCRIPTOR.type_bytes
            || program
                .output_type
                .canonical_bytes()
                .map_err(|_| R::Source)?
                .as_slice()
                != LanguageLexicalTokenProposal::PREPARED_DESCRIPTOR.type_bytes
        {
            return Err(R::Source);
        }
        let (source, source_receipt) =
            PreparedPortableExpressionEvaluator::new_with_storage_limits(
                &program,
                decode,
                limits.maximum_preparation_peak_bytes - add(base, raw_storage)?,
                limits.maximum_retained_bytes - base,
            )
            .map_err(|_| R::Source)?;
        let preparation = add(
            add(
                add(base, raw_storage)?,
                source_receipt.decoded_program_heap_bytes,
            )?,
            source_receipt.preparation_requested_bytes_bound,
        )?;
        admit(preparation, limits.maximum_preparation_peak_bytes)?;
        peak = peak.max(preparation);
        let retained = add(base, source_receipt.retained_heap_bytes_bound)?;
        admit(retained, limits.maximum_retained_bytes)?;
        let response = add(
            add(
                native.conversion_requested_bytes_bound,
                mul(limits.maximum_frame_bytes, 2)?,
            )?,
            size_of::<AdmittedLexicalTokenProposal>(),
        )?;
        admit(response, limits.maximum_response_requested_bytes)?;
        let admission = add(
            add(
                add(retained, mul(native.conversion_requested_bytes_bound, 2)?)?,
                dict.lookup_requested_bytes_bound,
            )?,
            add(
                mul(limits.maximum_frame_bytes, 2)?,
                size_of::<AdmittedLexicalTokenProposal>(),
            )?,
        )?;
        admit(admission, limits.maximum_admission_peak_bytes)?;
        let source_identity = semantic_digest("language/lexical-token-proposal-program@1", &raw);
        let receipt = LexicalProposerPortReceipt {
            retained_heap_bytes_bound: retained,
            preparation_peak_heap_bytes_bound: peak,
            admission_peak_heap_bytes_bound: admission,
            response_requested_bytes_bound: response,
            native,
            source: source_receipt,
            dictionary: dict,
        };
        Ok(Self {
            dictionary,
            definition,
            definition_identity,
            source_identity,
            source,
            family,
            maximum_frame_bytes: limits.maximum_frame_bytes,
            receipt,
        })
    }
    pub const fn storage_receipt(&self) -> LexicalProposerPortReceipt {
        self.receipt
    }
    pub fn canonical_definition(&self) -> &[u8] {
        &self.definition
    }
    pub fn admit_query(
        &mut self,
        canonical: &[u8],
    ) -> Result<AdmittedLexicalTokenProposal, LexicalProposerPortRefusal> {
        use LexicalProposerPortRefusal as R;
        admit(canonical.len(), self.maximum_frame_bytes)?;
        let view = validate_canonical_structured_value(canonical).map_err(|_| R::Frame)?;
        let query: LanguageLexicalProposalQuery =
            self.family.decode(canonical).map_err(|_| R::Native)?;
        let actual = view
            .record_field("definition")
            .map_err(|_| R::Frame)?
            .ok_or(R::Frame)?;
        let expected =
            validate_canonical_structured_value(&self.definition).map_err(|_| R::Frame)?;
        if actual.type_bytes() != expected.type_bytes()
            || actual.value_node() != expected.value_node()
        {
            return Err(R::Definition);
        }
        match query.origin() {
            LanguageLexicalProposalOrigin::Incomplete => {}
            LanguageLexicalProposalOrigin::Unknown(_) => {
                if self
                    .dictionary
                    .lookup(query.token().surface())
                    .map_err(|_| R::Resource)?
                    .is_some()
                {
                    return Err(R::Resource);
                }
            }
            LanguageLexicalProposalOrigin::Reviewed(origin) => {
                let lookup = self
                    .dictionary
                    .lookup(query.token().surface())
                    .map_err(|_| R::Resource)?
                    .ok_or(R::Resource)?;
                if !digest_matches(origin.dictionary_identity(), lookup.dictionary_identity())
                    || !digest_matches(origin.shard_identity(), lookup.shard_identity())
                    || *origin.shard() != u64::from(lookup.shard())
                    || *origin.entry() != u64::from(lookup.ordinal())
                    || query.reviewed_entry().as_ref() != Some(lookup.entry())
                {
                    return Err(R::Resource);
                }
            }
        }
        // Native query conversion and resource verification precede original Source
        // execution. The prepared evaluator retains its complete immutable program.
        let query_frame: Box<[u8]> = canonical.into();
        let output = self.source.evaluate(canonical).map_err(|_| R::Source)?;
        admit(output.len(), self.maximum_frame_bytes)?;
        let proposal: LanguageLexicalTokenProposal =
            self.family.decode(output).map_err(|_| R::Native)?;
        let output: Box<[u8]> = output.into();
        Ok(AdmittedLexicalTokenProposal {
            proposal,
            query: query_frame,
            output,
            definition_identity: self.definition_identity,
            source_identity: self.source_identity,
            dictionary_identity: self.dictionary.identity(),
            requested_bytes_bound: self.receipt.response_requested_bytes_bound,
        })
    }
}

/// Resource-backed token and whole original-revision proposal owners.
pub mod token_producer;
