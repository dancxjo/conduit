//! Whole original revision custody over the original per-token Source port.
use super::*;
use crate::lexical_scalar_scan::scan;
use crate::{
    LanguageLexicalProfile, LanguageLexicalProposedTape, LanguageLexicalTape, LanguageTextRevision,
    LinguisticTokenIdentity, TextSpan,
};
use alloc::vec::Vec;
#[derive(Clone, Copy, Debug)]
pub struct RevisionLimits {
    pub maximum_tokens: usize,
    pub maximum_retained_bytes: usize,
    pub maximum_preparation_peak_bytes: usize,
    pub maximum_admission_peak_bytes: usize,
    pub maximum_response_retained_bytes: usize,
}
#[derive(Clone, Copy, Debug)]
pub struct RevisionReceipt {
    pub retained_bytes_bound: usize,
    pub preparation_peak_bytes_bound: usize,
    pub admission_peak_bytes_bound: usize,
    pub response_retained_bytes_bound: usize,
}
/// Only this owner can publish an entire tape; no caller-supplied prior IDs.
pub struct AdmittedRevision {
    canonical: Box<[u8]>,
    tokens: Vec<AdmittedProducedToken>,
    definition: [u8; 32],
    dictionary: [u8; 32],
}
impl AdmittedRevision {
    pub fn canonical_proposed_tape(&self) -> &[u8] {
        &self.canonical
    }
    pub fn tokens(&self) -> &[AdmittedProducedToken] {
        &self.tokens
    }
    pub fn retained_bytes_bound(&self) -> usize {
        size_of::<Self>()
            + self.canonical.len()
            + self.tokens.capacity() * size_of::<AdmittedProducedToken>()
            + self
                .tokens
                .iter()
                .map(|t| t.retained_requested_bytes_bound())
                .sum::<usize>()
    }
}
struct RevisionRegistry {
    token: Composer,
    identity: Composer,
    ordinal: Composer,
    span: Composer,
    start: Composer,
    end: Composer,
    surface: Composer,
    prior: Composer,
    tape: Composer,
    proposed: Composer,
    tokens: Sequence,
    origins: Sequence,
}
pub struct PreparedRevisionProducer {
    pub(super) producer: PreparedTokenProducer,
    registry: RevisionRegistry,
    profile: Box<[u8]>,
    empty: Box<[u8]>,
    no_prior: Box<[u8]>,
    word: Box<[u8]>,
    punctuation: Box<[u8]>,
    complete: Box<[u8]>,
    partial: Box<[u8]>,
    basis: Box<[u8]>,
    limits: RevisionLimits,
    receipt: RevisionReceipt,
}
impl PreparedRevisionProducer {
    /// Full original proposer definition retained by the actual Source port.
    pub fn canonical_proposer_definition(&self) -> &[u8] {
        self.producer.port.canonical_definition()
    }

    pub fn prepare(
        producer: PreparedTokenProducer,
        limits: RevisionLimits,
    ) -> Result<Self, TokenProducerRefusal> {
        use TokenProducerRefusal as R;
        if limits.maximum_tokens > 128 {
            return Err(R::Capacity);
        }
        let port = &producer.port;
        let frame = port.maximum_frame_bytes;
        let specs: [(&'static NativeFamilyTypeDescriptor, &[&str]); 18] = [
            (LanguageLexicalToken::PREPARED_DESCRIPTOR, &[]),
            (LinguisticTokenIdentity::PREPARED_DESCRIPTOR, &[]),
            (LinguisticTokenIdentity::PREPARED_DESCRIPTOR, &["ordinal"]),
            (TextSpan::PREPARED_DESCRIPTOR, &[]),
            (TextSpan::PREPARED_DESCRIPTOR, &["start"]),
            (TextSpan::PREPARED_DESCRIPTOR, &["end"]),
            (LanguageLexicalToken::PREPARED_DESCRIPTOR, &["surface"]),
            (
                LanguageLexicalToken::PREPARED_DESCRIPTOR,
                &["prior_occurrence"],
            ),
            (LanguageLexicalTape::PREPARED_DESCRIPTOR, &[]),
            (LanguageLexicalProposedTape::PREPARED_DESCRIPTOR, &[]),
            (LanguageLexicalProfile::PREPARED_DESCRIPTOR, &[]),
            (LanguageLexicalToken::PREPARED_DESCRIPTOR, &["category"]),
            (LanguageLexicalToken::PREPARED_DESCRIPTOR, &["completeness"]),
            (TextSpan::PREPARED_DESCRIPTOR, &["basis"]),
            (
                LanguageLexicalToken::PREPARED_DESCRIPTOR,
                &["category", "word"],
            ),
            (
                LanguageLexicalToken::PREPARED_DESCRIPTOR,
                &["category", "punctuation"],
            ),
            (
                LanguageLexicalToken::PREPARED_DESCRIPTOR,
                &["completeness", "complete"],
            ),
            (TextSpan::PREPARED_DESCRIPTOR, &["basis", "unicode_scalar"]),
        ];
        let base = add(
            producer.receipt.retained_heap_bytes_bound,
            size_of::<Self>(),
        )?;
        let mut prep = add(base, mul(18, size_of::<Composer>())?)?;
        let mut retained = base;
        for (i, (d, p)) in specs.iter().enumerate() {
            let r = if i >= 14 {
                Composer::descriptor_steps_reservation(
                    &port.family,
                    d,
                    &[SchemaStep::Field(p[0]), SchemaStep::Case(p[1])],
                    frame,
                )
            } else {
                Composer::descriptor_reservation(&port.family, d, p, frame)
            }
            .map_err(|_| R::Descriptor)?;
            prep = add(prep, r.preparation_requested_bytes_bound)?;
            if i < 10 {
                retained = add(retained, r.retained_requested_bytes_bound)?;
            }
        }
        // Four sequence capacities (two retained, two temporary), eight constants,
        // all token histories, and two simultaneous original revision conversions.
        prep = add(prep, mul(frame, 12)?)?;
        retained = add(retained, mul(frame, 10)?)?;
        prep = prep.max(producer.receipt.preparation_peak_heap_bytes_bound);
        let response = add(
            add(size_of::<AdmittedRevision>(), frame)?,
            mul(
                limits.maximum_tokens,
                add(
                    producer.receipt.response_retained_bytes_bound,
                    size_of::<AdmittedProducedToken>(),
                )?,
            )?,
        )?;
        let conversion = port.receipt.native.conversion_requested_bytes_bound;
        let live_histories = add(mul(response, 2)?, frame)?;
        let admission = add(add(retained, live_histories)?, mul(conversion, 2)?)?.max(add(
            add(retained, live_histories)?,
            producer.receipt.admission_peak_heap_bytes_bound,
        )?);
        admit(retained, limits.maximum_retained_bytes)?;
        admit(prep, limits.maximum_preparation_peak_bytes)?;
        admit(admission, limits.maximum_admission_peak_bytes)?;
        admit(response, limits.maximum_response_retained_bytes)?;
        let cl = ParserCompositionLimits {
            maximum_output_bytes: frame,
            maximum_preparation_requested_bytes: usize::MAX,
            maximum_retained_requested_bytes: usize::MAX,
        };
        let mut composers = Vec::with_capacity(18);
        for (i, (d, p)) in specs.iter().enumerate() {
            composers.push(
                if i >= 14 {
                    Composer::prepare_descriptor_steps(
                        &port.family,
                        d,
                        &[SchemaStep::Field(p[0]), SchemaStep::Case(p[1])],
                        cl,
                    )
                } else {
                    Composer::prepare_descriptor_field(&port.family, d, p, cl)
                }
                .map_err(|_| R::Descriptor)?,
            );
        }
        let mut iter = composers.into_iter();
        let token = iter.next().ok_or(R::Descriptor)?;
        let identity = iter.next().ok_or(R::Descriptor)?;
        let ordinal = iter.next().ok_or(R::Descriptor)?;
        let span = iter.next().ok_or(R::Descriptor)?;
        let start = iter.next().ok_or(R::Descriptor)?;
        let end = iter.next().ok_or(R::Descriptor)?;
        let surface = iter.next().ok_or(R::Descriptor)?;
        let mut prior = iter.next().ok_or(R::Descriptor)?;
        let tape = iter.next().ok_or(R::Descriptor)?;
        let proposed = iter.next().ok_or(R::Descriptor)?;
        let mut profile_composer = iter.next().ok_or(R::Descriptor)?;
        let mut category = iter.next().ok_or(R::Descriptor)?;
        let mut completeness = iter.next().ok_or(R::Descriptor)?;
        let mut basis_composer = iter.next().ok_or(R::Descriptor)?;
        let mut word_unit = iter.next().ok_or(R::Descriptor)?;
        let mut punct_unit = iter.next().ok_or(R::Descriptor)?;
        let mut complete_unit = iter.next().ok_or(R::Descriptor)?;
        let mut basis_unit = iter.next().ok_or(R::Descriptor)?;
        let unit = view(word_unit.leaf(&[]).map_err(|_| R::Composition)?)?;
        let word: Box<[u8]> = category
            .variant("word", unit)
            .map_err(|_| R::Composition)?
            .into();
        let unit = view(punct_unit.leaf(&[]).map_err(|_| R::Composition)?)?;
        let punctuation = category
            .variant("punctuation", unit)
            .map_err(|_| R::Composition)?
            .into();
        let unit = view(complete_unit.leaf(&[]).map_err(|_| R::Composition)?)?;
        let complete = completeness
            .variant("complete", unit)
            .map_err(|_| R::Composition)?
            .into();
        let partial = completeness
            .variant("trailing_partial", unit)
            .map_err(|_| R::Composition)?
            .into();
        let no_prior = prior
            .variant("none", unit)
            .map_err(|_| R::Composition)?
            .into();
        let unit = view(basis_unit.leaf(&[]).map_err(|_| R::Composition)?)?;
        let basis = basis_composer
            .variant("unicode_scalar", unit)
            .map_err(|_| R::Composition)?
            .into();
        let mut empty_candidates = Sequence::prepare::<LanguageLexicalToken>(
            &port.family,
            &["candidates"],
            frame,
            frame,
            frame,
        )
        .map_err(|_| R::Descriptor)?;
        let empty = empty_candidates
            .compose(core::iter::empty())
            .map_err(|_| R::Composition)?
            .into();
        let mut entries = Sequence::prepare::<LanguageLexicalProfile>(
            &port.family,
            &["entries"],
            frame,
            frame,
            frame,
        )
        .map_err(|_| R::Descriptor)?;
        let entries = view(
            entries
                .compose(core::iter::empty())
                .map_err(|_| R::Composition)?,
        )?;
        let definition = view(&port.definition)?;
        let profile = profile_composer
            .record(&[
                entries,
                field(definition, "identity")?,
                field(definition, "language")?,
                field(definition, "provenance")?,
            ])
            .map_err(|_| R::Composition)?
            .into();
        let tokens = Sequence::prepare::<LanguageLexicalTape>(
            &port.family,
            &["tokens"],
            frame,
            frame,
            frame,
        )
        .map_err(|_| R::Descriptor)?;
        let origins = Sequence::prepare::<LanguageLexicalProposedTape>(
            &port.family,
            &["origins"],
            frame,
            frame,
            frame,
        )
        .map_err(|_| R::Descriptor)?;
        Ok(Self {
            producer,
            registry: RevisionRegistry {
                token,
                identity,
                ordinal,
                span,
                start,
                end,
                surface,
                prior,
                tape,
                proposed,
                tokens,
                origins,
            },
            profile,
            empty,
            no_prior,
            word,
            punctuation,
            complete,
            partial,
            basis,
            limits,
            receipt: RevisionReceipt {
                retained_bytes_bound: retained,
                preparation_peak_bytes_bound: prep,
                admission_peak_bytes_bound: admission,
                response_retained_bytes_bound: response,
            },
        })
    }
    pub const fn storage_receipt(&self) -> RevisionReceipt {
        self.receipt
    }
    pub fn propose(
        &mut self,
        canonical_revision: &[u8],
        previous: Option<&AdmittedRevision>,
        committed_prefix: u32,
    ) -> Result<AdmittedRevision, TokenProducerRefusal> {
        use TokenProducerRefusal as R;
        admit(
            canonical_revision.len(),
            self.producer.port.maximum_frame_bytes,
        )?;
        if previous.is_some_and(|p| {
            p.definition != self.producer.port.definition_identity
                || p.dictionary != self.producer.port.dictionary.identity()
        }) {
            return Err(R::Resource);
        }
        if let Some(previous) = previous {
            admit(
                previous.canonical.len(),
                self.producer.port.maximum_frame_bytes,
            )?;
            admit(
                previous.retained_bytes_bound(),
                self.receipt.response_retained_bytes_bound,
            )?;
            admit(previous.tokens.len(), self.limits.maximum_tokens)?;
            let actual = field(view(&previous.canonical)?, "definition")?;
            let expected = view(&self.producer.port.definition)?;
            if actual.type_bytes() != expected.type_bytes()
                || actual.value_node() != expected.value_node()
            {
                return Err(R::Resource);
            }
        }
        let current: LanguageTextRevision = self
            .producer
            .port
            .family
            .decode(canonical_revision)
            .map_err(|_| R::Frame)?;
        let old: Option<LanguageLexicalProposedTape> = previous
            .map(|p| {
                self.producer
                    .port
                    .family
                    .decode(&p.canonical)
                    .map_err(|_| R::Frame)
            })
            .transpose()?;
        crate::validate_text_revision(
            old.as_ref().map(|p| p.tape().source()),
            &current,
            committed_prefix,
            4096,
        )
        .map_err(|_| R::Token)?;
        if current.material().language().get().as_str() != self.producer.port.dictionary.language()
        {
            return Err(R::Resource);
        }
        let partial = matches!(current.finality(), crate::LanguageTextFinality::Partial);
        // Only borrowed canonical original source is used after both Native temporaries drop.
        drop(old);
        drop(current);
        let source = view(canonical_revision)?;
        let material = field(source, "material")?;
        let text =
            core::str::from_utf8(payload(field(material, "text")?)?).map_err(|_| R::Frame)?;
        let scanned = scan(text, partial, self.limits.maximum_tokens).map_err(|_| R::Token)?;
        let old_tape = previous
            .map(|p| field(view(&p.canonical)?, "tape"))
            .transpose()?;
        let old_stable = old_tape
            .map(|t| field(field(t, "source")?, "stable_prefix"))
            .transpose()?;
        let stable = match old_stable {
            Some(v) => match v.variant_payload("some").map_err(|_| R::Frame)? {
                Some(v) => u32::from_le_bytes(payload(v)?.try_into().map_err(|_| R::Frame)?),
                _ => 0,
            },
            None => 0,
        };
        let mut tokens = Vec::with_capacity(scanned.count);
        for (index, token) in scanned.tokens[..scanned.count].iter().enumerate() {
            let token = token.ok_or(R::Token)?;
            let ordinal = view(
                self.registry
                    .ordinal
                    .leaf(&(index as u64).to_le_bytes())
                    .map_err(|_| R::Composition)?,
            )?;
            let identity = view(
                self.registry
                    .identity
                    .record(&[
                        ordinal,
                        field(material, "identity")?,
                        field(material, "revision")?,
                    ])
                    .map_err(|_| R::Composition)?,
            )?;
            let start = view(
                self.registry
                    .start
                    .leaf(&u64::from(token.start).to_le_bytes())
                    .map_err(|_| R::Composition)?,
            )?;
            let end = view(
                self.registry
                    .end
                    .leaf(&u64::from(token.end).to_le_bytes())
                    .map_err(|_| R::Composition)?,
            )?;
            let span = view(
                self.registry
                    .span
                    .record(&[
                        view(&self.basis)?,
                        end,
                        start,
                        field(material, "identity")?,
                        field(material, "revision")?,
                    ])
                    .map_err(|_| R::Composition)?,
            )?;
            let surface = view(
                self.registry
                    .surface
                    .leaf(token.surface.as_bytes())
                    .map_err(|_| R::Composition)?,
            )?;
            let mut prior_identity = None;
            if token.end <= stable {
                if let Some(tape) = old_tape {
                    for old in field(tape, "tokens")?
                        .collection_elements()
                        .map_err(|_| R::Frame)?
                    {
                        let old = old.map_err(|_| R::Frame)?;
                        let old_span = field(old, "span")?;
                        let old_start = u64::from_le_bytes(
                            payload(field(old_span, "start")?)?
                                .try_into()
                                .map_err(|_| R::Frame)?,
                        );
                        let old_end = u64::from_le_bytes(
                            payload(field(old_span, "end")?)?
                                .try_into()
                                .map_err(|_| R::Frame)?,
                        );
                        if old_start == u64::from(token.start)
                            && old_end == u64::from(token.end)
                            && payload(field(old, "surface")?)? == token.surface.as_bytes()
                            && field(old, "completeness")?
                                .variant_payload("complete")
                                .map_err(|_| R::Frame)?
                                .is_some()
                        {
                            prior_identity = Some(field(old, "identity")?);
                            break;
                        }
                    }
                }
            }
            let prior = match prior_identity {
                Some(id) => view(
                    self.registry
                        .prior
                        .variant("some", id)
                        .map_err(|_| R::Composition)?,
                )?,
                None => view(&self.no_prior)?,
            };
            let base = self
                .registry
                .token
                .record(&[
                    view(&self.empty)?,
                    view(if token.word {
                        &self.word
                    } else {
                        &self.punctuation
                    })?,
                    view(if token.partial {
                        &self.partial
                    } else {
                        &self.complete
                    })?,
                    identity,
                    prior,
                    span,
                    surface,
                ])
                .map_err(|_| R::Composition)?;
            tokens.push(self.producer.propose(base)?);
        }
        let mut token_views = [None; 128];
        let mut origin_views = [None; 128];
        for (i, token) in tokens.iter().enumerate() {
            let q = view(token.proposal.canonical_query())?;
            token_views[i] = Some(field(q, "token")?);
            origin_views[i] = Some(field(q, "origin")?);
        }
        let token_frame = view(
            self.registry
                .tokens
                .compose(
                    token_views[..tokens.len()]
                        .iter()
                        .map(|v| v.expect("filled bounded slot")),
                )
                .map_err(|_| R::Composition)?,
        )?;
        let origin_frame = view(
            self.registry
                .origins
                .compose(
                    origin_views[..tokens.len()]
                        .iter()
                        .map(|v| v.expect("filled bounded slot")),
                )
                .map_err(|_| R::Composition)?,
        )?;
        let tape = view(
            self.registry
                .tape
                .record(&[view(&self.profile)?, source, token_frame])
                .map_err(|_| R::Composition)?,
        )?;
        let canonical = self
            .registry
            .proposed
            .record(&[view(&self.producer.port.definition)?, origin_frame, tape])
            .map_err(|_| R::Composition)?;
        let native: LanguageLexicalProposedTape = self
            .producer
            .port
            .family
            .decode(canonical)
            .map_err(|_| R::Token)?;
        drop(native);
        let response = AdmittedRevision {
            canonical: canonical.into(),
            tokens,
            definition: self.producer.port.definition_identity,
            dictionary: self.producer.port.dictionary.identity(),
        };
        admit(
            response.retained_bytes_bound(),
            self.receipt.response_retained_bytes_bound,
        )?;
        Ok(response)
    }
}
