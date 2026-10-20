//! Opt-in ordinary-kernel numerical dispatch; no public score/token acceptance entrance.
use super::super::learned::ParserRoundModel;
use super::*;
use conduit_ai::integer_categorical_step::{channel::*, PreparedCategoricalStep};
use conduit_composite::KernelCompositeDefinition;
use conduit_core::{PreparedStructuredComposer, PreparedStructuredValueValidator};
use conduit_plot::{CheckedPlot, ProfileCatalog};
pub struct CursorNumericLease {
    pub(super) identity: Arc<RoundIdentity>,
    pub(super) slot: usize,
    pub(super) choice: u64,
    pub(super) ordinal: usize,
    pub(super) source: usize,
    pub(super) invocation: u64,
}
impl CursorNumericLease {
    pub fn context(&self) -> &[String; 5] {
        &self.identity.context
    }
    pub fn generation(&self) -> u64 {
        self.identity.generation
    }
    pub fn survivor_slot(&self) -> usize {
        self.slot
    }
    pub fn lexical_choice(&self) -> u64 {
        self.choice
    }
    pub fn request_ordinal(&self) -> usize {
        self.ordinal
    }
    pub fn source_ordinal(&self) -> usize {
        self.source
    }
    pub fn invocation(&self) -> u64 {
        self.invocation
    }
    pub fn prior_canonical(&self) -> &[u8] {
        &self.identity.prior
    }
}
pub struct CursorNumericalEvidence {
    pub(super) lease: CursorNumericLease,
    pub(super) numerical: DispatchedCategoricalInvocation,
    pub(super) selection: ParserRoundModel,
    pub(super) profile: Arc<PreparedCategoricalStep>,
    pub(super) definition: Arc<KernelCompositeDefinition>,
}
impl CursorNumericalEvidence {
    pub fn request(&self) -> &CursorNumericLease {
        &self.lease
    }
    pub fn profile(&self) -> &PreparedCategoricalStep {
        &self.profile
    }
    pub fn definition(&self) -> &KernelCompositeDefinition {
        &self.definition
    }
    pub fn numerical(&self) -> &DispatchedCategoricalInvocation {
        &self.numerical
    }
    pub fn selection(&self) -> ParserRoundModel {
        self.selection
    }
    pub fn source_ordinal(&self) -> usize {
        self.lease.source
    }
    pub fn survivor_slot(&self) -> usize {
        self.lease.slot
    }
    pub fn lexical_choice(&self) -> u64 {
        self.lease.choice
    }
}
pub struct PreparedCursorLearnedChannel {
    identity: Arc<()>,
    definition: Arc<KernelCompositeDefinition>,
    numerical: PreparedCategoricalChannel<CursorNumericLease>,
    profile: Arc<PreparedCategoricalStep>,
    selection: ParserRoundModel,
    next: u64,
    held: Option<CursorNumericLease>,
    stopped: bool,
    context: Option<[String; 5]>,
    composer: PreparedStructuredComposer,
    output_shape: PreparedStructuredValueValidator,
    input: Vec<u8>,
    output: Vec<u8>,
}
impl PreparedCursorLearnedChannel {
    pub fn prepare(
        plot: &CheckedPlot,
        definition: KernelCompositeDefinition,
        profile: Arc<PreparedCategoricalStep>,
        selection: ParserRoundModel,
        capacity: usize,
    ) -> Result<Self, FallbackRoundRefusal> {
        validate_model(&profile, selection)?;
        let retained_definition = Arc::new(definition.clone());
        let numerical =
            PreparedCategoricalChannel::prepare(plot, definition, profile.clone(), capacity)
                .map_err(|_| FallbackRoundRefusal::Model)?;
        Self::finish_prepare(numerical, profile, selection, retained_definition)
    }
    pub fn prepare_canonical(
        document: &CheckedSyntaxDocument,
        entry: &str,
        catalog: &ProfileCatalog,
        definition: KernelCompositeDefinition,
        profile: Arc<PreparedCategoricalStep>,
        selection: ParserRoundModel,
        capacity: usize,
    ) -> Result<Self, FallbackRoundRefusal> {
        validate_model(&profile, selection)?;
        let retained_definition = Arc::new(definition.clone());
        let numerical = PreparedCategoricalChannel::prepare_canonical(
            document,
            entry,
            catalog,
            definition,
            profile.clone(),
            capacity,
        )
        .map_err(|_| FallbackRoundRefusal::Model)?;
        Self::finish_prepare(numerical, profile, selection, retained_definition)
    }
    fn finish_prepare(
        numerical: PreparedCategoricalChannel<CursorNumericLease>,
        profile: Arc<PreparedCategoricalStep>,
        selection: ParserRoundModel,
        definition: Arc<KernelCompositeDefinition>,
    ) -> Result<Self, FallbackRoundRefusal> {
        let composer = PreparedStructuredComposer::new(profile.indices_type(), BYTES)
            .map_err(|_| FallbackRoundRefusal::Model)?;
        let output_shape = PreparedStructuredValueValidator::new(profile.scores_type(), BYTES)
            .map_err(|_| FallbackRoundRefusal::Model)?;
        let mut output = buffer();
        output.resize(BYTES, 0);
        Ok(Self {
            identity: Arc::new(()),
            definition,
            numerical,
            profile,
            selection,
            next: 1,
            held: None,
            stopped: false,
            context: None,
            composer,
            output_shape,
            input: buffer(),
            output,
        })
    }
    pub(super) fn stop_hosted(&mut self) {
        self.stopped = true;
    }
    pub fn is_stopped(&self) -> bool {
        self.stopped
    }
    pub fn pending(&self) -> Option<&CursorNumericLease> {
        self.held.as_ref().or_else(|| self.numerical.pending())
    }
    pub fn numerical(&self) -> &PreparedCategoricalChannel<CursorNumericLease> {
        &self.numerical
    }
    pub fn cancel(&mut self) -> Result<(), FallbackRoundRefusal> {
        self.stopped = true;
        self.numerical
            .cancel()
            .map_err(FallbackRoundRefusal::NumericalChannel)
    }
    /// One actual kernel step, with ingress only after an exact private pending request.
    pub fn step(&mut self, cursor: &mut PreparedRoundCursor<'_>) -> R<bool> {
        // Reject foreign/stale request ownership before touching either real Play.
        if !cursor
            .numeric_binding
            .as_ref()
            .is_some_and(|identity| Arc::ptr_eq(identity, &self.identity))
            || self
                .pending()
                .is_some_and(|pending| !Arc::ptr_eq(&pending.identity, &cursor.identity))
            || self
                .context
                .as_ref()
                .is_some_and(|context| context != &cursor.identity.context)
            || cursor.machine.invocation != self.next
        {
            return Err(CursorRefusal::ForeignLease);
        }
        let result = self.step_inner(cursor);
        if let Err(reason) = &result {
            if *reason != CursorRefusal::Pressure {
                self.stopped = true;
                cursor.failure = Some(reason.clone());
                cursor.stage = Stage::Failed;
            }
        }
        result
    }
    fn step_inner(&mut self, cursor: &mut PreparedRoundCursor<'_>) -> R<bool> {
        if !cursor
            .numeric_binding
            .as_ref()
            .is_some_and(|identity| Arc::ptr_eq(identity, &self.identity))
        {
            return Err(CursorRefusal::ForeignLease);
        }
        if self.stopped
            || self.numerical.is_closed()
            || !matches!(cursor.stage, Stage::Numerical)
            || cursor.failure.is_some()
        {
            return Err(CursorRefusal::Lifecycle);
        }
        if self
            .context
            .as_ref()
            .is_some_and(|c| c != &cursor.identity.context)
            || cursor.machine.invocation != self.next
        {
            return Err(CursorRefusal::ForeignLease);
        }
        if cursor.numerical.len() >= learned::MAX_LEARNED_REQUESTS_PER_ROUND
            || self.next > learned::MAX_LEARNED_REQUESTS_PER_EPOCH
        {
            return Err(CursorRefusal::Bounds);
        }
        if let Some(pending) = self.pending() {
            if !Arc::ptr_eq(&pending.identity, &cursor.identity) {
                return Err(CursorRefusal::ForeignLease);
            }
        }
        if self.numerical.pending().is_none() {
            if self.held.is_none() {
                let indices = member(&cursor.machine.features, &["indices"])?;
                if indices
                    .collection_length()
                    .map_err(|_| CursorRefusal::Canonical)?
                    != 25
                {
                    return Err(CursorRefusal::Model);
                }
                let placeholder = indices
                    .collection_index(0)
                    .map_err(|_| CursorRefusal::Canonical)?
                    .ok_or(CursorRefusal::Canonical)?;
                let mut values = [placeholder; 25];
                for (index, value) in values.iter_mut().enumerate() {
                    *value = indices
                        .collection_index(index as u16)
                        .map_err(|_| CursorRefusal::Canonical)?
                        .ok_or(CursorRefusal::Canonical)?;
                }
                put(
                    &mut self.input,
                    self.composer
                        .collection(&values)
                        .map_err(|_| CursorRefusal::Model)?,
                )?;
                put(&mut cursor.machine.numerical_input, &self.input)?;
                let (slot, choice) = cursor.machine.branch_identity();
                self.held = Some(CursorNumericLease {
                    identity: cursor.identity.clone(),
                    slot,
                    choice,
                    ordinal: cursor.numerical.len(),
                    source: cursor
                        .io
                        .len()
                        .checked_sub(1)
                        .ok_or(CursorRefusal::Lifecycle)?,
                    invocation: self.next,
                });
            }
            let lease = self.held.take().ok_or(CursorRefusal::Lifecycle)?;
            match self.numerical.submit(lease, &self.input) {
                Ok(CategoricalChannelSubmission::Accepted) => (),
                Ok(CategoricalChannelSubmission::TerminalRetained(reason)) => {
                    return Err(CursorRefusal::NumericalChannel(reason))
                }
                Err(failure) => {
                    let pressure = failure.refusal == CategoricalChannelRefusal::Pressure;
                    self.held = Some(failure.lease);
                    return Err(if pressure {
                        CursorRefusal::Pressure
                    } else {
                        CursorRefusal::NumericalChannel(failure.refusal)
                    });
                }
            }
        }
        let Some(completed) = self
            .numerical
            .step()
            .map_err(CursorRefusal::NumericalChannel)?
        else {
            return Ok(false);
        };
        let (lease, numerical) = completed.into_parts();
        cursor.numerical.push(CursorNumericalEvidence {
            lease,
            numerical,
            selection: self.selection,
            profile: self.profile.clone(),
            definition: self.definition.clone(),
        });
        let evidence = cursor.numerical.last().ok_or(CursorRefusal::Numerical)?;
        let (slot, choice) = cursor.machine.branch_identity();
        if !Arc::ptr_eq(&evidence.lease.identity, &cursor.identity)
            || evidence.lease.slot != slot
            || evidence.lease.choice != choice
            || evidence.lease.ordinal + 1 != cursor.numerical.len()
            || evidence.lease.invocation != self.next
            || evidence.numerical.invocation() != self.next
        {
            return Err(CursorRefusal::ForeignLease);
        }
        let input_length = evidence.numerical.input_len();
        if input_length > self.output.len() {
            return Err(CursorRefusal::Bounds);
        }
        evidence
            .numerical
            .copy_input_into(&mut self.output[..input_length])
            .map_err(|_| CursorRefusal::Numerical)?;
        if self.output[..input_length] != cursor.machine.numerical_input {
            return Err(CursorRefusal::ForeignLease);
        }
        let length = evidence.numerical.output_len();
        if length > self.output.len() {
            return Err(CursorRefusal::Bounds);
        }
        evidence
            .numerical
            .copy_output_into(&mut self.output[..length])
            .map_err(|_| CursorRefusal::Numerical)?;
        self.output_shape
            .validate(&self.output[..length])
            .map_err(|_| CursorRefusal::Model)?;
        put(&mut cursor.machine.numerical_scores, &self.output[..length])?;
        cursor.machine.numerical_complete()?;
        self.next = self.next.checked_add(1).ok_or(CursorRefusal::Bounds)?;
        cursor.stage = Stage::Build;
        Ok(true)
    }
}
impl<'d> PreparedRoundCursor<'d> {
    pub fn prepare_learned(
        owner: &AnalysisEpochOwner<'_>,
        document: &'d CheckedSyntaxDocument,
        programs: &PreparedFallbackRoundPrograms,
        channel: &mut PreparedCursorLearnedChannel,
    ) -> R<Self> {
        if channel.stopped
            || channel.pending().is_some()
            || channel.numerical.is_closed()
            || channel
                .context
                .as_ref()
                .is_some_and(|c| c != &round_context(owner))
        {
            return Err(CursorRefusal::Lifecycle);
        }
        let mut result = Self::prepare(
            owner,
            document,
            programs,
            AnalysisRoundMode::AuthenticatedCategorical,
            channel.next,
        )?;
        result.numeric_binding = Some(channel.identity.clone());
        if channel.context.is_none() {
            channel.context = Some(round_context(owner));
        }
        Ok(result)
    }
    pub fn numerical_evidence(&self) -> &[CursorNumericalEvidence] {
        &self.numerical
    }
}
fn validate_model(
    profile: &PreparedCategoricalStep,
    selection: ParserRoundModel,
) -> Result<(), FallbackRoundRefusal> {
    use crate::{parser_model::*, parser_punctuation_model::*};
    let (artifact, signature, bytes) = match selection {
        ParserRoundModel::AcceptedEnglishV2 => {
            let (a, s) = english_parser_scorer_descriptor();
            (a, s, ENGLISH_PARSER_WEIGHTS)
        }
        ParserRoundModel::UnacceptedPunctuationV3 => {
            let (a, s) = punctuation_parser_scorer_descriptor();
            (a, s, PUNCTUATION_PARSER_WEIGHTS)
        }
    };
    let selected = profile
        .resource()
        .signature()
        .semantic_digest()
        .map_err(|_| FallbackRoundRefusal::Model)?;
    let required = signature
        .semantic_digest()
        .map_err(|_| FallbackRoundRefusal::Model)?;
    if profile.dimensions() != ENGLISH_PARSER_DIMENSIONS
        || profile.resource().artifact() != &artifact
        || selected != required
        || profile.resource().bytes() != bytes
    {
        return Err(FallbackRoundRefusal::Model);
    }
    Ok(())
}
