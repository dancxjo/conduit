//! Resource-backed token proposals. Original revision custody is supplied by
//! the revision owner, never inferred from a token ID or span.
use super::{CanonicalLexicalTokenProposal, PreparedLexicalProposerPort};
use crate::lexical_canonical_sequence::PreparedLexicalCanonicalSequence as Sequence;
use crate::parser_canonical_composition::{
    ParserCompositionLimits, PreparedParserCanonicalComposer as Composer,
};
use crate::parser_canonical_schema::SchemaStep;
use crate::{
    LanguageLexicalCandidate, LanguageLexicalCompleteness, LanguageLexicalProposalOrigin,
    LanguageLexicalProposalQuery, LanguageLexicalReviewedOrigin, LanguageLexicalToken,
    LanguageLexicalUnknownOrigin,
};
use alloc::boxed::Box;
use conduit_core::{
    validate_canonical_structured_value as validate, ValidatedCanonicalStructuredValue as View,
};
use conduit_plot::rust_binding::{NativeFamilyTypeDescriptor, PreparedNativeRustBinding};
use core::mem::size_of;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenProducerRefusal {
    Capacity,
    Descriptor,
    Frame,
    Token,
    Composition,
    Resource,
    Source,
}
#[derive(Clone, Copy, Debug)]
pub struct TokenProducerLimits {
    pub maximum_retained_bytes: usize,
    pub maximum_preparation_peak_bytes: usize,
    pub maximum_admission_peak_bytes: usize,
    pub maximum_response_retained_bytes: usize,
}
#[derive(Clone, Copy, Debug)]
pub struct TokenProducerReceipt {
    pub retained_heap_bytes_bound: usize,
    pub preparation_peak_heap_bytes_bound: usize,
    pub admission_peak_heap_bytes_bound: usize,
    pub response_retained_bytes_bound: usize,
}
pub struct AdmittedProducedToken {
    original: Box<[u8]>,
    proposal: CanonicalLexicalTokenProposal,
}
impl AdmittedProducedToken {
    pub fn original_token(&self) -> &[u8] {
        &self.original
    }
    pub fn proposal(&self) -> &CanonicalLexicalTokenProposal {
        &self.proposal
    }
    pub fn retained_requested_bytes_bound(&self) -> usize {
        size_of::<Self>() + self.original.len() + self.proposal.retained_requested_bytes_bound()
    }
}
struct Registry {
    token: Composer,
    query: Composer,
    reviewed: Composer,
    origin: Composer,
    optional_entry: Composer,
    number0: Composer,
    number1: Composer,
    shard: Composer,
    candidates: Sequence,
}
pub struct PreparedTokenProducer {
    port: PreparedLexicalProposerPort,
    registry: Registry,
    unknown: Box<[u8]>,
    incomplete: Box<[u8]>,
    unknown_candidates: Box<[u8]>,
    empty_candidates: Box<[u8]>,
    no_entry: Box<[u8]>,
    receipt: TokenProducerReceipt,
}
fn add(a: usize, b: usize) -> Result<usize, TokenProducerRefusal> {
    a.checked_add(b).ok_or(TokenProducerRefusal::Capacity)
}
fn mul(a: usize, b: usize) -> Result<usize, TokenProducerRefusal> {
    a.checked_mul(b).ok_or(TokenProducerRefusal::Capacity)
}
fn admit(value: usize, limit: usize) -> Result<(), TokenProducerRefusal> {
    if value > limit {
        Err(TokenProducerRefusal::Capacity)
    } else {
        Ok(())
    }
}
fn view(bytes: &[u8]) -> Result<View<'_>, TokenProducerRefusal> {
    validate(bytes).map_err(|_| TokenProducerRefusal::Frame)
}
fn field<'a>(value: View<'a>, name: &str) -> Result<View<'a>, TokenProducerRefusal> {
    value
        .record_field(name)
        .map_err(|_| TokenProducerRefusal::Frame)?
        .ok_or(TokenProducerRefusal::Frame)
}
fn payload<'a>(value: View<'a>) -> Result<&'a [u8], TokenProducerRefusal> {
    let node = value.value_node();
    if node.first() != Some(&0) || node.len() < 5 {
        return Err(TokenProducerRefusal::Frame);
    }
    Ok(&node[5..])
}
fn hex(digest: [u8; 32]) -> [u8; 64] {
    let mut out = [0; 64];
    let hex = b"0123456789abcdef";
    for (i, value) in digest.iter().enumerate() {
        out[2 * i] = hex[usize::from(value >> 4)];
        out[2 * i + 1] = hex[usize::from(value & 15)];
    }
    out
}
impl PreparedTokenProducer {
    pub fn prepare(
        port: PreparedLexicalProposerPort,
        limits: TokenProducerLimits,
    ) -> Result<Self, TokenProducerRefusal> {
        use TokenProducerRefusal as R;
        let frame = port.maximum_frame_bytes;
        let specs: [(&'static NativeFamilyTypeDescriptor, &[&str]); 12] = [
            (LanguageLexicalToken::PREPARED_DESCRIPTOR, &[]),
            (LanguageLexicalProposalQuery::PREPARED_DESCRIPTOR, &[]),
            (LanguageLexicalReviewedOrigin::PREPARED_DESCRIPTOR, &[]),
            (LanguageLexicalProposalOrigin::PREPARED_DESCRIPTOR, &[]),
            (
                LanguageLexicalProposalQuery::PREPARED_DESCRIPTOR,
                &["reviewed_entry"],
            ),
            (
                LanguageLexicalReviewedOrigin::PREPARED_DESCRIPTOR,
                &["entry"],
            ),
            (
                LanguageLexicalReviewedOrigin::PREPARED_DESCRIPTOR,
                &["shard"],
            ),
            (
                LanguageLexicalReviewedOrigin::PREPARED_DESCRIPTOR,
                &["shard_identity"],
            ),
            (
                LanguageLexicalToken::PREPARED_DESCRIPTOR,
                &["category", "word"],
            ),
            (LanguageLexicalUnknownOrigin::PREPARED_DESCRIPTOR, &[]),
            (LanguageLexicalCandidate::PREPARED_DESCRIPTOR, &[]),
            (LanguageLexicalCandidate::PREPARED_DESCRIPTOR, &["lemma"]),
        ];
        let base = add(port.receipt.retained_heap_bytes_bound, size_of::<Self>())?;
        let mut preparation = base;
        let mut retained = base;
        for (index, (descriptor, path)) in specs.iter().enumerate() {
            let r = if index == 8 {
                Composer::descriptor_steps_reservation(
                    &port.family,
                    descriptor,
                    &[SchemaStep::Field("category"), SchemaStep::Case("word")],
                    frame,
                )
            } else {
                Composer::descriptor_reservation(&port.family, descriptor, path, frame)
            }
            .map_err(|_| R::Descriptor)?;
            preparation = add(preparation, r.preparation_requested_bytes_bound)?;
            if index < 8 {
                retained = add(retained, r.retained_requested_bytes_bound)?;
            }
        }
        // Two sequence buffers, four temporary candidate frames and five retained
        // constant frames are admitted before the first composer allocation.
        preparation = add(preparation, mul(frame, 11)?)?;
        preparation = preparation.max(port.receipt.preparation_peak_heap_bytes_bound);
        retained = add(retained, mul(frame, 6)?)?;
        let admission = add(
            add(
                port.receipt.admission_peak_heap_bytes_bound,
                retained - port.receipt.retained_heap_bytes_bound,
            )?,
            add(frame, size_of::<AdmittedProducedToken>())?,
        )?;
        let response = add(
            mul(frame, 3)?,
            add(
                size_of::<AdmittedProducedToken>(),
                size_of::<CanonicalLexicalTokenProposal>(),
            )?,
        )?;
        admit(preparation, limits.maximum_preparation_peak_bytes)?;
        admit(retained, limits.maximum_retained_bytes)?;
        admit(admission, limits.maximum_admission_peak_bytes)?;
        admit(response, limits.maximum_response_retained_bytes)?;
        let build = |index: usize| {
            let limits = ParserCompositionLimits {
                maximum_output_bytes: frame,
                maximum_preparation_requested_bytes: preparation,
                maximum_retained_requested_bytes: retained,
            };
            if index == 8 {
                Composer::prepare_descriptor_steps(
                    &port.family,
                    specs[index].0,
                    &[SchemaStep::Field("category"), SchemaStep::Case("word")],
                    limits,
                )
            } else {
                Composer::prepare_descriptor_field(
                    &port.family,
                    specs[index].0,
                    specs[index].1,
                    limits,
                )
            }
            .map_err(|_| R::Composition)
        };
        let token = build(0)?;
        let query = build(1)?;
        let reviewed = build(2)?;
        let mut origin = build(3)?;
        let mut optional_entry = build(4)?;
        let number0 = build(5)?;
        let number1 = build(6)?;
        let shard = build(7)?;
        let mut unit = build(8)?;
        let mut unknown_record = build(9)?;
        let mut candidate = build(10)?;
        let mut lemma = build(11)?;
        let mut candidates = Sequence::prepare::<LanguageLexicalToken>(
            &port.family,
            &["candidates"],
            frame,
            frame,
            frame,
        )
        .map_err(|_| R::Composition)?;
        let mut morphology = Sequence::prepare::<LanguageLexicalCandidate>(
            &port.family,
            &["morphology"],
            frame,
            frame,
            frame,
        )
        .map_err(|_| R::Composition)?;
        let unit = view(unit.leaf(&[]).map_err(|_| R::Composition)?)?;
        let incomplete: Box<[u8]> = origin
            .variant("incomplete", unit)
            .map_err(|_| R::Composition)?
            .into();
        let no_entry: Box<[u8]> = optional_entry
            .variant("none", unit)
            .map_err(|_| R::Composition)?
            .into();
        let definition = view(&port.definition)?;
        let policy = field(definition, "unknown")?;
        let unknown_record = view(
            unknown_record
                .record(&[field(policy, "identity")?, field(policy, "provenance")?])
                .map_err(|_| R::Composition)?,
        )?;
        let unknown: Box<[u8]> = origin
            .variant("unknown", unknown_record)
            .map_err(|_| R::Composition)?
            .into();
        let empty_candidates: Box<[u8]> = candidates
            .compose(core::iter::empty())
            .map_err(|_| R::Composition)?
            .into();
        let lemma = view(lemma.leaf(b"<unknown>").map_err(|_| R::Composition)?)?;
        let morphology = view(
            morphology
                .compose(core::iter::empty())
                .map_err(|_| R::Composition)?,
        )?;
        let mut frames: [Option<Box<[u8]>>; 4] = [None, None, None, None];
        let mut count = 0;
        for pos in field(policy, "alternatives")?
            .collection_elements()
            .map_err(|_| R::Frame)?
        {
            let pos = pos.map_err(|_| R::Frame)?;
            if count == 4 {
                return Err(R::Token);
            }
            frames[count] = Some(
                candidate
                    .record(&[lemma, morphology, pos])
                    .map_err(|_| R::Composition)?
                    .into(),
            );
            count += 1;
        }
        let mut values = [None; 4];
        for index in 0..count {
            values[index] = Some(view(frames[index].as_deref().ok_or(R::Frame)?)?);
        }
        let unknown_candidates: Box<[u8]> = candidates
            .compose(
                values[..count]
                    .iter()
                    .map(|value| value.expect("filled bounded candidate slot")),
            )
            .map_err(|_| R::Composition)?
            .into();
        let registry = Registry {
            token,
            query,
            reviewed,
            origin,
            optional_entry,
            number0,
            number1,
            shard,
            candidates,
        };
        Ok(Self {
            port,
            registry,
            unknown,
            incomplete,
            unknown_candidates,
            empty_candidates,
            no_entry,
            receipt: TokenProducerReceipt {
                retained_heap_bytes_bound: retained,
                preparation_peak_heap_bytes_bound: preparation,
                admission_peak_heap_bytes_bound: admission,
                response_retained_bytes_bound: response,
            },
        })
    }
    pub const fn storage_receipt(&self) -> TokenProducerReceipt {
        self.receipt
    }
    pub fn propose(
        &mut self,
        canonical_token: &[u8],
    ) -> Result<AdmittedProducedToken, TokenProducerRefusal> {
        use TokenProducerRefusal as R;
        admit(canonical_token.len(), self.port.maximum_frame_bytes)?;
        let token = view(canonical_token)?;
        let native: LanguageLexicalToken = self
            .port
            .family
            .decode(canonical_token)
            .map_err(|_| R::Token)?;
        if !native.candidates().is_empty() {
            return Err(R::Token);
        }
        let partial = matches!(
            native.completeness(),
            LanguageLexicalCompleteness::TrailingPartial
        );
        drop(native);
        let surface =
            core::str::from_utf8(payload(field(token, "surface")?)?).map_err(|_| R::Frame)?;
        let lookup = if partial {
            None
        } else {
            self.port
                .dictionary
                .lookup(surface)
                .map_err(|_| R::Resource)?
        };
        let definition = view(&self.port.definition)?;
        let (candidate_bytes, origin_bytes, entry_bytes) = if partial {
            (&*self.empty_candidates, &*self.incomplete, &*self.no_entry)
        } else if let Some(lookup) = lookup.as_ref() {
            let entry = view(lookup.canonical_entry())?;
            let mut values = [None; 4];
            let mut count = 0;
            for value in field(entry, "candidates")?
                .collection_elements()
                .map_err(|_| R::Frame)?
            {
                if count == 4 {
                    return Err(R::Token);
                }
                values[count] = Some(value.map_err(|_| R::Frame)?);
                count += 1;
            }
            let candidates = self
                .registry
                .candidates
                .compose(
                    values[..count]
                        .iter()
                        .map(|value| value.expect("filled bounded candidate slot")),
                )
                .map_err(|_| R::Composition)?;
            let index = view(
                self.registry
                    .number0
                    .leaf(&u64::from(lookup.ordinal()).to_le_bytes())
                    .map_err(|_| R::Composition)?,
            )?;
            let shard = view(
                self.registry
                    .number1
                    .leaf(&u64::from(lookup.shard()).to_le_bytes())
                    .map_err(|_| R::Composition)?,
            )?;
            let identity = view(
                self.registry
                    .shard
                    .leaf(&hex(lookup.shard_identity()))
                    .map_err(|_| R::Composition)?,
            )?;
            let origin = view(
                self.registry
                    .reviewed
                    .record(&[
                        field(definition, "dictionary_identity")?,
                        index,
                        shard,
                        identity,
                    ])
                    .map_err(|_| R::Composition)?,
            )?;
            let origin = self
                .registry
                .origin
                .variant("reviewed", origin)
                .map_err(|_| R::Composition)?;
            let optional = self
                .registry
                .optional_entry
                .variant("some", entry)
                .map_err(|_| R::Composition)?;
            (candidates, origin, optional)
        } else {
            (&*self.unknown_candidates, &*self.unknown, &*self.no_entry)
        };
        let new_token = view(
            self.registry
                .token
                .record(&[
                    view(candidate_bytes)?,
                    field(token, "category")?,
                    field(token, "completeness")?,
                    field(token, "identity")?,
                    field(token, "prior_occurrence")?,
                    field(token, "span")?,
                    field(token, "surface")?,
                ])
                .map_err(|_| R::Composition)?,
        )?;
        let query = self
            .registry
            .query
            .record(&[
                definition,
                view(origin_bytes)?,
                view(entry_bytes)?,
                new_token,
            ])
            .map_err(|_| R::Composition)?;
        // Release the lookup's temporary Native entry before Source input/output
        // conversion. The composed query owns complete original entry bytes.
        drop(lookup);
        let proposal = self
            .port
            .admit_query(query)
            .map_err(|_| R::Source)?
            .into_canonical_receipt();
        Ok(AdmittedProducedToken {
            original: canonical_token.into(),
            proposal,
        })
    }
}

#[cfg(all(feature = "parser-model-selection", target_has_atomic = "ptr"))]
#[path = "token_producer/model_definition.rs"]
pub mod model_definition;
#[path = "token_producer/revision.rs"]
pub mod revision;
