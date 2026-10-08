//! Finite per-revision canonical ownership, prepared before any target ingress.
//! References locate complete retained frames; they confer no standalone fact,
//! commitment or frontier authority. Only Session orchestration publishes a book.
use crate::{
    lexical::PreparedLexicalTape,
    parser_session_candidate_admission::ParserStableCandidateAdmission,
    parser_session_canonical_ingress::PreparedParserExecutionFrames,
    parser_session_fixed_ingress::{ParserFixedFrames, ParserFixedHistory},
    parser_session_historical_base::ParserSessionHistoricalBase,
    parser_session_mixed_custody::ParserMixedHistory,
    parser_session_numeric_custody::ParserNumericFrames,
    parser_session_profile::lexical_tape_storage,
    parser_session_seed_admission::ParserSeedBeamAdmission,
    LanguageLexicalTape,
};
use alloc::{rc::Rc, vec::Vec};
use conduit_plot::rust_binding::{PreparedNativeFamily, PreparedNativeRustBinding};
use core::mem::{align_of, size_of};

#[derive(Clone, Copy, Debug)]
pub(crate) struct RevisionStorageLimits {
    pub(crate) maximum_source_executions: u32,
    pub(crate) maximum_model_executions: u32,
    pub(crate) source_input_bytes: usize,
    pub(crate) source_output_bytes: usize,
    pub(crate) model_input_bytes: usize,
    pub(crate) model_output_bytes: usize,
    pub(crate) maximum_retained_chain_bytes: usize,
    pub(crate) maximum_preparation_peak_bytes: usize,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct RevisionStorageReceipt {
    pub(crate) original_tape_heap_bytes: usize,
    pub(crate) newly_reserved_bytes_bound: usize,
    pub(crate) complete_retained_chain_bytes_bound: usize,
    pub(crate) preparation_peak_bytes_bound: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RevisionStorageRefusal {
    Limits,
    Overflow,
    Pressure,
    Allocation,
    OriginalTape,
    Consumed,
}
pub(crate) struct ReservedMixedFrames {
    pub(crate) source: PreparedParserExecutionFrames,
    pub(crate) numeric: ParserNumericFrames,
}
/// Exact execution/admission order. These indices only locate the complete
/// retained material in this immutable book; an event is never fact authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ParserRevisionEvent {
    Source(usize),
    Model(usize),
    SeedAdmission { source_execution: usize },
    StableAdmission(usize),
}
/// This owner retains the actual opaque lexical producer, full canonical tape,
/// original model/profile and every accepted Source/model frame. A prior book
/// retains all earlier commitments and origins independently of future revisions.
pub(crate) struct ParserRevisionCustody {
    pub(crate) lexical: Rc<PreparedLexicalTape>,
    pub(crate) original_lexical_bytes: Rc<Vec<u8>>,
    pub(crate) base: Rc<ParserSessionHistoricalBase>,
    pub(crate) previous: Option<Rc<ParserRevisionCustody>>,
    pub(crate) source_histories: Vec<ParserFixedHistory>,
    pub(crate) mixed_histories: Vec<ParserMixedHistory>,
    pub(crate) stable_admissions: Vec<ParserStableCandidateAdmission>,
    pub(crate) seed_admission: Option<ParserSeedBeamAdmission>,
    pub(crate) events: Vec<ParserRevisionEvent>,
    source_frames: Vec<ParserFixedFrames>,
    mixed_frames: Vec<ReservedMixedFrames>,
    pub(crate) storage: RevisionStorageReceipt,
    pub(crate) published: bool,
}
fn add(a: usize, b: usize) -> Result<usize, RevisionStorageRefusal> {
    a.checked_add(b).ok_or(RevisionStorageRefusal::Overflow)
}
fn mul(a: usize, b: usize) -> Result<usize, RevisionStorageRefusal> {
    a.checked_mul(b).ok_or(RevisionStorageRefusal::Overflow)
}
fn reserve<T>(count: usize) -> Result<Vec<T>, RevisionStorageRefusal> {
    let mut v = Vec::new();
    v.try_reserve_exact(count)
        .map_err(|_| RevisionStorageRefusal::Allocation)?;
    Ok(v)
}
enum LexicalStorage {
    New(PreparedLexicalTape),
    Existing(Rc<PreparedLexicalTape>, Rc<Vec<u8>>),
}
impl LexicalStorage {
    fn tape(&self) -> &PreparedLexicalTape {
        match self {
            Self::New(tape) => tape,
            Self::Existing(tape, _) => tape,
        }
    }
}
impl ParserRevisionCustody {
    /// Every cumulative requested buffer/header and the complete existing tape
    /// are admitted before generated Native readmission or the first allocation.
    pub(crate) fn prepare(
        lexical: PreparedLexicalTape,
        original_lexical_bytes: &[u8],
        base: Rc<ParserSessionHistoricalBase>,
        previous: Option<Rc<Self>>,
        family: &mut PreparedNativeFamily,
        limits: RevisionStorageLimits,
    ) -> Result<Rc<Self>, RevisionStorageRefusal> {
        Self::prepare_inner(
            LexicalStorage::New(lexical),
            original_lexical_bytes,
            base,
            previous,
            family,
            limits,
        )
    }
    /// A commitment-only stage shares the already admitted original producer and
    /// full canonical tape. Its previous immutable book owns that storage once.
    pub(crate) fn prepare_existing(
        previous: Rc<Self>,
        family: &mut PreparedNativeFamily,
        limits: RevisionStorageLimits,
    ) -> Result<Rc<Self>, RevisionStorageRefusal> {
        let lexical = LexicalStorage::Existing(
            previous.lexical.clone(),
            previous.original_lexical_bytes.clone(),
        );
        let original = previous.original_lexical_bytes.clone();
        Self::prepare_inner(
            lexical,
            original.as_slice(),
            previous.base.clone(),
            Some(previous),
            family,
            limits,
        )
    }
    fn prepare_inner(
        lexical: LexicalStorage,
        original_lexical_bytes: &[u8],
        base: Rc<ParserSessionHistoricalBase>,
        previous: Option<Rc<Self>>,
        family: &mut PreparedNativeFamily,
        limits: RevisionStorageLimits,
    ) -> Result<Rc<Self>, RevisionStorageRefusal> {
        use RevisionStorageRefusal as R;
        let source_count =
            usize::try_from(limits.maximum_source_executions).map_err(|_| R::Limits)?;
        let model_count =
            usize::try_from(limits.maximum_model_executions).map_err(|_| R::Limits)?;
        if source_count == 0
            || !family.contains_descriptor(LanguageLexicalTape::PREPARED_DESCRIPTOR)
        {
            return Err(R::Limits);
        }
        for bound in [
            limits.source_input_bytes,
            limits.source_output_bytes,
            limits.model_input_bytes,
            limits.model_output_bytes,
        ] {
            if bound == 0 || bound > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES {
                return Err(R::Limits);
            }
        }
        if lexical.tape().tape().tokens().len() > usize::from(base.capabilities.maximum_tokens()) {
            return Err(R::OriginalTape);
        }
        if original_lexical_bytes.len() > limits.source_input_bytes
            || lexical.tape().tape().profile() != base.model.expected_lexical_profile()
        {
            return Err(R::OriginalTape);
        }
        let is_new = matches!(&lexical, LexicalStorage::New(_));
        if is_new {
            crate::lexical::validate_prepared_lexical_successor(
                previous.as_ref().map(|prior| prior.lexical.as_ref()),
                lexical.tape(),
            )
            .map_err(|_| R::OriginalTape)?;
        }
        if let LexicalStorage::Existing(tape, bytes) = &lexical {
            let prior = previous.as_ref().ok_or(R::OriginalTape)?;
            if !Rc::ptr_eq(tape, &prior.lexical)
                || !Rc::ptr_eq(bytes, &prior.original_lexical_bytes)
            {
                return Err(R::OriginalTape);
            }
        }
        let tape = if is_new {
            lexical_tape_storage(lexical.tape().tape()).map_err(|_| R::Overflow)?
        } else {
            0
        };
        let owner = add(
            add(size_of::<Self>(), 2 * size_of::<usize>())?,
            4 * align_of::<Self>(),
        )?;
        let event_count = add(source_count, model_count)?;
        let event_headers = mul(event_count, size_of::<ParserRevisionEvent>())?;
        let source_headers = mul(
            source_count,
            add(
                size_of::<ParserFixedFrames>(),
                add(
                    size_of::<ParserFixedHistory>(),
                    size_of::<ParserStableCandidateAdmission>(),
                )?,
            )?,
        )?;
        let model_headers = mul(
            model_count,
            add(
                size_of::<ReservedMixedFrames>(),
                size_of::<ParserMixedHistory>(),
            )?,
        )?;
        let source_buffers = mul(
            source_count,
            add(limits.source_input_bytes, limits.source_output_bytes)?,
        )?;
        // Each model invocation retains its complete Source feature ingress and
        // numeric index, score and wrapped Native output frames.
        let model_buffers = mul(
            model_count,
            add(
                add(limits.source_input_bytes, limits.model_input_bytes)?,
                add(mul(limits.model_input_bytes, 2)?, limits.model_output_bytes)?,
            )?,
        )?;
        let tape_owner = if is_new {
            add(
                add(size_of::<PreparedLexicalTape>(), 2 * size_of::<usize>())?,
                4 * align_of::<PreparedLexicalTape>(),
            )?
        } else {
            0
        };
        let bytes_owner = if is_new {
            add(
                add(size_of::<Vec<u8>>(), 2 * size_of::<usize>())?,
                4 * align_of::<Vec<u8>>(),
            )?
        } else {
            0
        };
        let newly = add(
            add(tape_owner, bytes_owner)?,
            add(
                add(
                    add(add(add(owner, tape)?, source_headers)?, model_headers)?,
                    source_buffers,
                )?,
                add(
                    model_buffers,
                    if is_new {
                        original_lexical_bytes.len()
                    } else {
                        0
                    },
                )?,
            )?,
        )?;
        let newly = add(newly, event_headers)?;
        if previous
            .as_ref()
            .is_some_and(|prior| !prior.published || !Rc::ptr_eq(&prior.base, &base))
        {
            return Err(R::OriginalTape);
        }
        let prior = previous
            .as_ref()
            .map_or(base.storage.complete_base_bytes_bound, |p| {
                p.storage.complete_retained_chain_bytes_bound
            });
        let chain = add(prior, newly)?;
        let peak = add(
            chain,
            family.storage_receipt().conversion_requested_bytes_bound,
        )?;
        if chain > limits.maximum_retained_chain_bytes
            || peak > limits.maximum_preparation_peak_bytes
        {
            return Err(R::Pressure);
        }
        // Readmission compares every original field, including producer profile,
        // token occurrence/span/prior correspondence, uncertainty and revision.
        if family
            .decode::<LanguageLexicalTape>(original_lexical_bytes)
            .map_err(|_| R::OriginalTape)?
            != *lexical.tape().tape()
        {
            return Err(R::OriginalTape);
        }
        let (lexical, original) = match lexical {
            LexicalStorage::New(tape) => {
                let mut original = reserve(original_lexical_bytes.len())?;
                original.extend_from_slice(original_lexical_bytes);
                (Rc::new(tape), Rc::new(original))
            }
            LexicalStorage::Existing(tape, bytes) => (tape, bytes),
        };
        let mut source_frames = reserve(source_count)?;
        for _ in 0..source_count {
            source_frames.push(
                ParserFixedFrames::prepare(limits.source_input_bytes, limits.source_output_bytes)
                    .map_err(|_| R::Allocation)?,
            );
        }
        let mut mixed_frames = reserve(model_count)?;
        for _ in 0..model_count {
            let source = PreparedParserExecutionFrames::prepare(
                limits.source_input_bytes,
                limits.model_input_bytes,
            )
            .map_err(|_| R::Allocation)?;
            let indices = reserve(limits.model_input_bytes)?;
            let scores = reserve(limits.model_input_bytes)?;
            let mut output = reserve(limits.model_output_bytes)?;
            output.resize(limits.model_output_bytes, 0);
            mixed_frames.push(ReservedMixedFrames {
                source,
                numeric: ParserNumericFrames {
                    feature_guard: Vec::new(),
                    indices,
                    scores,
                    output,
                },
            });
        }
        let events = reserve(event_count)?;
        let source_histories = reserve(source_count)?;
        let mixed_histories = reserve(model_count)?;
        let stable_admissions = reserve(source_count)?;
        let result = Self {
            lexical,
            original_lexical_bytes: original,
            base,
            previous,
            source_histories,
            mixed_histories,
            stable_admissions,
            seed_admission: None,
            events,
            source_frames,
            mixed_frames,
            storage: RevisionStorageReceipt {
                original_tape_heap_bytes: tape,
                newly_reserved_bytes_bound: newly,
                complete_retained_chain_bytes_bound: chain,
                preparation_peak_bytes_bound: peak,
            },
            published: false,
        };
        // Allocated capacities are checked before exposing any ingress. Header
        // and frame allocations move between pools and histories without growth.
        let mut actual = add(add(add(owner, tape)?, tape_owner)?, bytes_owner)?;
        actual = add(
            actual,
            if is_new {
                result.original_lexical_bytes.capacity()
            } else {
                0
            },
        )?;
        actual = add(
            actual,
            mul(
                result.source_frames.capacity(),
                size_of::<ParserFixedFrames>(),
            )?,
        )?;
        actual = add(
            actual,
            mul(
                result.mixed_frames.capacity(),
                size_of::<ReservedMixedFrames>(),
            )?,
        )?;
        actual = add(
            actual,
            mul(
                result.source_histories.capacity(),
                size_of::<ParserFixedHistory>(),
            )?,
        )?;
        actual = add(
            actual,
            mul(
                result.mixed_histories.capacity(),
                size_of::<ParserMixedHistory>(),
            )?,
        )?;
        actual = add(
            actual,
            mul(
                result.stable_admissions.capacity(),
                size_of::<ParserStableCandidateAdmission>(),
            )?,
        )?;
        actual = add(
            actual,
            mul(result.events.capacity(), size_of::<ParserRevisionEvent>())?,
        )?;
        for frame in &result.source_frames {
            actual = add(actual, frame.retained_bytes().ok_or(R::Overflow)?)?;
        }
        for frame in &result.mixed_frames {
            actual = add(
                actual,
                add(
                    frame.source.retained_capacity_bytes(),
                    frame.numeric.retained_capacity_bytes().ok_or(R::Overflow)?,
                )?,
            )?;
        }
        if actual > newly {
            return Err(R::Pressure);
        }
        Ok(Rc::new(result))
    }
    /// Checks the complete ordered locator tape against retained material. Full
    /// Source/Native replay is additional and separately reserved; this check
    /// never treats a locator, count or equal ID as an authorization witness.
    pub(crate) fn source_parent(
        &self,
        link: &crate::parser_session_fixed_ingress::ParserSourceParentLink,
        before: usize,
    ) -> Option<&ParserFixedHistory> {
        let mut book = self;
        for _ in 0..link.prior_revisions {
            book = book.previous.as_deref()?;
            if !book.published {
                return None;
            }
        }
        if link.prior_revisions == 0 && link.execution >= before {
            return None;
        }
        book.source_histories.get(link.execution)
    }
    pub(crate) fn validate_event_order(&self) -> Result<(), RevisionStorageRefusal> {
        let mut source = 0usize;
        let mut model = 0usize;
        let mut stable = 0usize;
        let mut seed = false;
        for event in &self.events {
            match *event {
                ParserRevisionEvent::Source(index) => {
                    if index != source || self.source_histories.get(index).is_none() {
                        return Err(RevisionStorageRefusal::OriginalTape);
                    }
                    let history = &self.source_histories[index];
                    if let Some(parent) = history.rank_parent {
                        if history.entry
                            != crate::parser_session_execution::ParserSessionEntry::ScoreProposal
                            || parent.model_execution >= model
                            || parent.mask_execution >= source
                            || !self.rank_parent_matches(parent, &history.input)
                        {
                            return Err(RevisionStorageRefusal::OriginalTape);
                        }
                    }
                    for link in history.parent_links.iter().flatten() {
                        if self
                            .source_parent(link, index)
                            .is_none_or(|parent| !link.matches(&parent.output, &history.input))
                        {
                            return Err(RevisionStorageRefusal::OriginalTape);
                        }
                    }
                    source = add(source, 1)?;
                }
                ParserRevisionEvent::Model(index) => {
                    if !seed || index != model || self.mixed_histories.get(index).is_none() {
                        return Err(RevisionStorageRefusal::OriginalTape);
                    }
                    model = add(model, 1)?;
                }
                ParserRevisionEvent::SeedAdmission { source_execution } => {
                    if seed
                        || source_execution >= source
                        || self
                            .seed_admission
                            .as_ref()
                            .is_none_or(|admission| admission.seed_execution != source_execution)
                        || self
                            .source_histories
                            .get(source_execution)
                            .is_none_or(|origin| {
                                origin.entry
                                    != crate::parser_session_execution::ParserSessionEntry::Seed
                            })
                    {
                        return Err(RevisionStorageRefusal::OriginalTape);
                    }
                    seed = true;
                }
                ParserRevisionEvent::StableAdmission(index) => {
                    if index != stable
                        || self
                            .stable_admissions
                            .get(index)
                            .is_none_or(|admission| admission.stable_proposal_execution >= source)
                    {
                        return Err(RevisionStorageRefusal::OriginalTape);
                    }
                    stable = add(stable, 1)?;
                }
            }
        }
        if source != self.source_histories.len()
            || model != self.mixed_histories.len()
            || stable != self.stable_admissions.len()
            || seed != self.seed_admission.is_some()
        {
            return Err(RevisionStorageRefusal::OriginalTape);
        }
        Ok(())
    }
    pub(crate) fn rank_parent_matches(
        &self,
        parent: crate::parser_session_fixed_ingress::ParserRankParent,
        query: &[u8],
    ) -> bool {
        use crate::parser_session_execution::ParserSessionEntry;
        let Some(model) = self.mixed_histories.get(parent.model_execution) else {
            return false;
        };
        let Some(mask) = self.source_histories.get(parent.mask_execution) else {
            return false;
        };
        if !matches!(
            mask.entry,
            ParserSessionEntry::LegalMask | ParserSessionEntry::IndependentMask
        ) {
            return false;
        }
        let Ok(scores) = conduit_core::validate_canonical_structured_value(&model.numeric.output)
        else {
            return false;
        };
        let Ok(mask) = conduit_core::validate_canonical_structured_value(&mask.output) else {
            return false;
        };
        let Ok(scored) = conduit_core::validate_canonical_structured_value(query) else {
            return false;
        };
        let mut rank = crate::parser_session_rank::PreparedParserDriverRank::<
            crate::parser_session_numeric_profile::PinnedFourSlotNumericProfile,
        >::new();
        rank.matches_selected(scores, mask, scored, parent.selected_ordinal) == Ok(true)
    }
    pub(crate) fn source_frame(&mut self) -> Result<ParserFixedFrames, RevisionStorageRefusal> {
        if self.published {
            return Err(RevisionStorageRefusal::Consumed);
        }
        self.source_frames
            .pop()
            .ok_or(RevisionStorageRefusal::Pressure)
    }
    pub(crate) fn model_frames(&mut self) -> Result<ReservedMixedFrames, RevisionStorageRefusal> {
        if self.published {
            return Err(RevisionStorageRefusal::Consumed);
        }
        self.mixed_frames
            .pop()
            .ok_or(RevisionStorageRefusal::Pressure)
    }
    pub(crate) fn retain_source(
        &mut self,
        history: ParserFixedHistory,
    ) -> Result<usize, RevisionStorageRefusal> {
        if !self.base.contains_plan(&history.original_plan) {
            return Err(RevisionStorageRefusal::OriginalTape);
        }
        if self.published
            || self.source_histories.len() == self.source_histories.capacity()
            || self.events.len() == self.events.capacity()
        {
            return Err(RevisionStorageRefusal::Pressure);
        }
        let index = self.source_histories.len();
        self.source_histories.push(history);
        self.events.push(ParserRevisionEvent::Source(index));
        Ok(index)
    }
    pub(crate) fn retain_seed_admission(
        &mut self,
        admission: ParserSeedBeamAdmission,
    ) -> Result<(), RevisionStorageRefusal> {
        if self.published
            || self.seed_admission.is_some()
            || self.events.len() == self.events.capacity()
        {
            return Err(RevisionStorageRefusal::Pressure);
        }
        let source_execution = admission.seed_execution;
        if self
            .source_histories
            .get(source_execution)
            .is_none_or(|origin| {
                origin.entry != crate::parser_session_execution::ParserSessionEntry::Seed
            })
        {
            return Err(RevisionStorageRefusal::OriginalTape);
        }
        self.seed_admission = Some(admission);
        self.events
            .push(ParserRevisionEvent::SeedAdmission { source_execution });
        Ok(())
    }
    pub(crate) fn retain_stable_admission(
        &mut self,
        admission: ParserStableCandidateAdmission,
    ) -> Result<usize, RevisionStorageRefusal> {
        if self.published
            || self.stable_admissions.len() == self.stable_admissions.capacity()
            || self.events.len() == self.events.capacity()
        {
            return Err(RevisionStorageRefusal::Pressure);
        }
        if self
            .source_histories
            .get(admission.stable_proposal_execution)
            .is_none_or(|origin| {
                origin.entry != crate::parser_session_execution::ParserSessionEntry::StableFact
            })
        {
            return Err(RevisionStorageRefusal::OriginalTape);
        }
        let index = self.stable_admissions.len();
        self.stable_admissions.push(admission);
        self.events
            .push(ParserRevisionEvent::StableAdmission(index));
        Ok(index)
    }
    pub(crate) fn retain_model(
        &mut self,
        history: ParserMixedHistory,
    ) -> Result<usize, RevisionStorageRefusal> {
        if !self.base.contains_plan(&history.original_source_plan)
            || !self.base.contains_plan(&history.numeric.original_plan)
            || !alloc::sync::Arc::ptr_eq(
                self.base.model.prepared_categorical(),
                &history.numeric.original_model,
            )
        {
            return Err(RevisionStorageRefusal::OriginalTape);
        }
        if self.published
            || self.mixed_histories.len() == self.mixed_histories.capacity()
            || self.events.len() == self.events.capacity()
        {
            return Err(RevisionStorageRefusal::Pressure);
        }
        let index = self.mixed_histories.len();
        self.mixed_histories.push(history);
        self.events.push(ParserRevisionEvent::Model(index));
        Ok(index)
    }
}
