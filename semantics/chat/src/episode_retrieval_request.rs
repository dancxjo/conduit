//! Final role-separated model input assembled from one verified snapshot.
//! The trusted preparation-time projector owns current-evidence formatting;
//! this module never substitutes a new fact projection or a second lookup.
use alloc::vec::Vec;
use conduit_core::{validate_canonical_structured_value, ValidatedCanonicalStructuredValue};
#[path = "episode_retrieval_request/pending.rs"]
mod pending;
#[path = "episode_retrieval_request/writer.rs"]
mod writer;
use writer::*;
pub const EPISODE_RETRIEVAL_DATA_SCHEMA: &str = "conduit.chat/episode-retrieval-request@2";
pub const MAXIMUM_EPISODE_RETRIEVAL_REQUEST_BYTES: usize = 4096;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpisodeCurrentProjectionRefusal {
    Malformed,
    Unsupported,
    ByteBound,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpisodeRetrievalRequestRefusal {
    InstructionBound,
    SnapshotMismatch,
    CurrentProjection(EpisodeCurrentProjectionRefusal),
    InvalidCurrentJson,
    InvalidFragmentJson,
    MalformedExperience,
    ByteBound,
    PendingBound,
    MalformedPending,
}
use EpisodeRetrievalRequestRefusal as R;
/// The projector is a trusted prepared profile, not input-supplied code. It
/// must validate/project the exact EpisodeExperience packet passed to it and
/// execute without allocation or work beyond its admitted preparation bounds.
/// Resource references remain inert; this callback grants no read authority.
pub struct PreparedEpisodeRetrievalRequest<P> {
    instruction: Vec<u8>,
    projector: P,
    scratch: [u8; MAXIMUM_EPISODE_RETRIEVAL_REQUEST_BYTES],
    output: Vec<u8>,
    pending: pending::PreparedEpisodeRetrievalPending,
}
impl<P> PreparedEpisodeRetrievalRequest<P>
where
    P: FnMut(&[u8], &mut [u8]) -> Result<usize, EpisodeCurrentProjectionRefusal>,
{
    pub fn prepare(instruction: &str, projector: P) -> Result<Self, R> {
        if instruction.is_empty() || instruction.len() > 1024 {
            return Err(R::InstructionBound);
        }
        let mut quoted = Vec::with_capacity(MAXIMUM_EPISODE_RETRIEVAL_REQUEST_BYTES);
        string(&mut quoted, instruction.as_bytes())?;
        Ok(Self {
            instruction: quoted,
            projector,
            scratch: [0; MAXIMUM_EPISODE_RETRIEVAL_REQUEST_BYTES],
            output: Vec::with_capacity(MAXIMUM_EPISODE_RETRIEVAL_REQUEST_BYTES),
            pending: pending::PreparedEpisodeRetrievalPending::prepare(),
        })
    }
    pub fn output(&self) -> &[u8] {
        &self.output
    }
    /// Exact native pending episode paired atomically with `output`.
    pub fn pending(&self) -> &[u8] {
        self.pending.output()
    }
    /// Full-wire overflow refuses atomically. Context selection must separately
    /// reserve room for current evidence, instruction, and envelope overhead.
    pub fn compose(
        &mut self,
        snapshot: &crate::BorrowedEpisodeRetrievalSnapshot<'_>,
        context: &crate::PreparedEpisodeRetrievalContext,
    ) -> Result<&[u8], R> {
        self.output.clear();
        self.pending.clear();
        if let Err(error) = self.compose_inner(snapshot, context) {
            self.output.clear();
            self.pending.clear();
            return Err(error);
        }
        Ok(&self.output)
    }
    fn compose_inner(
        &mut self,
        snapshot: &crate::BorrowedEpisodeRetrievalSnapshot<'_>,
        context: &crate::PreparedEpisodeRetrievalContext,
    ) -> Result<(), R> {
        if context.selected_snapshot_identity() != Some(snapshot.identity()) {
            return Err(R::SnapshotMismatch);
        }
        let input = snapshot.input();
        self.scratch.fill(0);
        let length =
            (self.projector)(input.experience, &mut self.scratch).map_err(R::CurrentProjection)?;
        if length > self.scratch.len() {
            return Err(R::ByteBound);
        }
        let current = &self.scratch[..length];
        conduit_web::validate_json_text(current).map_err(|_| R::InvalidCurrentJson)?;
        let experience = validate_canonical_structured_value(input.experience)
            .map_err(|_| R::MalformedExperience)?;
        let observation = field(experience, "observation")?;
        let out = &mut self.output;
        put(
            out,
            br#"{"schema":"conduit.ai/generation-context@1","instruction":"#,
        )?;
        put(out, &self.instruction)?;
        put(out, br#","data":{"schema":"#)?;
        string(out, EPISODE_RETRIEVAL_DATA_SCHEMA.as_bytes())?;
        put(out, br#","body_id":"#)?;
        string(out, input.expected.body_id.as_bytes())?;
        put(out, br#","wake_id":"#)?;
        string(out, input.expected.wake_id.as_bytes())?;
        put(out, br#","wake_sequence":"#)?;
        integer(out, input.expected.wake_sequence)?;
        put(out, br#","snapshot_identity":"#)?;
        put(out, b"\"")?;
        const HEX: &[u8; 16] = b"0123456789abcdef";
        for byte in snapshot.identity() {
            put(out, &[HEX[(byte >> 4) as usize], HEX[(byte & 15) as usize]])?
        }
        put(out, br#"","reference":"#)?;
        instant(out, input.reference)?;
        put(out, br#","retrieved_at":"#)?;
        if input.retrieved == input.reference {
            put(out, br#""same_as_reference""#)?;
        } else {
            instant(out, input.retrieved)?;
        }
        put(out, br#","clock_semantics":"#)?;
        string(
            out,
            match input.clock_basis {
                conduit_ai::BorrowedTemporalClockBasis::UnixEpochMilliseconds => b"unix_epoch",
                conduit_ai::BorrowedTemporalClockBasis::MonotonicMilliseconds(_) => {
                    b"named_monotonic"
                }
            },
        )?;
        put(out, br#","observation":{"identity":"#)?;
        string(out, leaf(field(observation, "identity")?)?)?;
        put(out, br#","content":"#)?;
        string(out, leaf(field(observation, "content")?)?)?;
        put(out, br#","retain":"#)?;
        match leaf(field(observation, "retain")?)? {
            [0] => put(out, b"false")?,
            [1] => put(out, b"true")?,
            _ => return Err(R::MalformedExperience),
        }
        put(out, br#"},"current_experience":"#)?;
        put(out, current)?;
        put(out, br#","recalled":["#)?;
        for (index, fragment) in context.selected_fragments().enumerate() {
            conduit_web::validate_json_text(fragment).map_err(|_| R::InvalidFragmentJson)?;
            if index != 0 {
                put(out, b",")?
            }
            put(out, fragment)?;
        }
        put(out, b"],")?;
        diagnostics(out, context)?;
        put(out, b"}}")?;
        self.pending.encode(snapshot, context, current)?;
        Ok(())
    }
}
fn instant(out: &mut Vec<u8>, instant: conduit_core::BorrowedTemporalInstant<'_>) -> Result<(), R> {
    // Snapshot admission has already established this exact ms/unit profile.
    put(out, br#"{"ticks":"#)?;
    integer(out, instant.ticks)?;
    put(out, br#","scale":"milliseconds","clock_basis":"#)?;
    string(out, instant.clock_basis.as_bytes())?;
    put(out, br#","resolution_ticks":"#)?;
    integer(out, instant.resolution_ticks)?;
    put(out, br#","uncertainty_ticks":"#)?;
    integer(out, instant.uncertainty_ticks)?;
    put(out, b"}")
}
type View<'a> = ValidatedCanonicalStructuredValue<'a>;
fn field<'a>(view: View<'a>, name: &str) -> Result<View<'a>, R> {
    view.record_field(name)
        .map_err(|_| R::MalformedExperience)?
        .ok_or(R::MalformedExperience)
}
fn leaf(view: View<'_>) -> Result<&[u8], R> {
    let node = view.value_node();
    if node.len() < 5 || node[0] != 0 {
        return Err(R::MalformedExperience);
    }
    let size =
        u32::from_le_bytes(node[1..5].try_into().map_err(|_| R::MalformedExperience)?) as usize;
    if size != node.len() - 5 {
        return Err(R::MalformedExperience);
    }
    Ok(&node[5..])
}

// These are actual selector outcomes, not an inference from recalled.len().
fn diagnostics(
    out: &mut Vec<u8>,
    context: &crate::PreparedEpisodeRetrievalContext,
) -> Result<(), R> {
    use crate::{
        EpisodeRetrievalResolution as Resolution, EpisodeTemporalOmissionReason as Temporal,
    };
    use conduit_ai::ContextOmissionReason as Context;
    put(out, br#""retrieval":{"relevance_profile":"#)?;
    match context.relevance_profile() {
        Some(profile) => string(out, profile.as_bytes())?,
        None => put(out, b"null")?,
    }
    put(out, br#","relevance_omitted_items":"#)?;
    integer(out, context.relevance_omitted_items() as u64)?;
    if !context.irrelevant_episodes().is_empty() {
        put(out, br#","irrelevant_episodes":["#)?;
        for (index, episode) in context.irrelevant_episodes().iter().enumerate() {
            if index != 0 {
                put(out, b",")?;
            }
            integer(out, *episode)?;
        }
        put(out, b"]")?;
    }
    put(out, br#","temporal":"#)?;
    match context.temporal_resolution() {
        Some(resolution) => string(
            out,
            match resolution {
                Resolution::Selected => b"selected",
                Resolution::NoHistory => b"no_history",
                Resolution::NeedEarlierHistory => b"need_earlier_history",
                Resolution::BoundaryUnavailable => b"boundary_unavailable",
            },
        )?,
        None => put(out, b"null")?,
    }
    put(out, br#","context":"#)?;
    string(
        out,
        match context.selection() {
            None if context.relevance_profile().is_some()
                && !context.irrelevant_episodes().is_empty() =>
            {
                b"no_relevant_history_current_only"
            }
            None => b"no_temporal_selection",
            Some(selection) if selection.selected_len == 0 => b"all_omitted_current_only",
            Some(selection) if selection.omitted_len > 0 => b"selected_with_omissions",
            Some(_) => b"selected",
        },
    )?;

    put(out, br#","temporal_omissions":["#)?;
    for (index, omission) in context.temporal_omissions().enumerate() {
        if index != 0 {
            put(out, b",")?;
        }
        put(out, br#"{"episode":"#)?;
        integer(out, omission.episode)?;
        put(out, br#","item":"#)?;
        match omission.item {
            Some(item) => integer(out, u64::from(item))?,
            None => put(out, b"null")?,
        }
        put(out, br#","reason":"#)?;
        string(
            out,
            match omission.reason {
                Temporal::MissingEvidence => b"missing_evidence",
                Temporal::MissingObservedTime => b"missing_observed_time",
                Temporal::UnsupportedTime => b"unsupported_time",
                Temporal::ClockMismatch => b"clock_mismatch",
                Temporal::InvalidInterval => b"invalid_interval",
                Temporal::FutureSource => b"future_source",
            },
        )?;
        put(out, b"}")?;
    }
    put(out, br#"],"context_omissions":["#)?;
    for (index, (episode, reason)) in context.omissions().enumerate() {
        if index != 0 {
            put(out, b",")?;
        }
        put(out, br#"{"episode":"#)?;
        integer(out, episode)?;
        put(out, br#","reason":"#)?;
        string(
            out,
            match reason {
                Context::ReviewedRedundancy => b"reviewed_redundancy",
                Context::ItemBudget => b"item_budget",
                Context::ByteBudget => b"byte_budget",
                Context::TokenBudget => b"token_budget",
                Context::WorkBudget => b"work_budget",
            },
        )?;
        put(out, b"}")?;
    }
    put(out, b"]}")
}
