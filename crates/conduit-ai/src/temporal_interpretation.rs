//! Bounded model-derived temporal intent, resolved only against supplied time truth.

use alloc::{format, string::String, vec::Vec};
use conduit_core::{
    LocalDate, LocalDateTime, LocalTime, MeetingCandidate, MeetingProposalRequest, NamedTimeZone,
    TemporalBoundary, TemporalInstant, TemporalScale, TemporalWindow, ZonedResolution,
    MAXIMUM_TEMPORAL_IDENTITY_BYTES, UNIX_UTC_CLOCK_BASIS,
};
use serde::{Deserialize, Serialize};

use crate::{
    llm_contract, ModelDerivedResult, ModelResultDisposition, ModelResultInvalidity,
    ModelResultProvenance, LLM_INTERPRET_KIND,
};

pub const MAXIMUM_TEMPORAL_INTENT_BYTES: usize = 2_048;
pub const MAXIMUM_TEMPORAL_PARTICIPANTS: usize = 8;
pub const MAXIMUM_TEMPORAL_EXCLUSIONS: usize = 32;
pub const MAXIMUM_TEMPORAL_AMBIGUITIES: usize = 16;
pub const MAXIMUM_TEMPORAL_CANDIDATES: u16 = 32;
pub const MAXIMUM_RELATIVE_DAYS: i16 = 366;
pub const MAXIMUM_DURATION_MINUTES: u16 = 24 * 60;
pub const MAXIMUM_RECURRENCE_OCCURRENCES: u16 = 32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalInterpretationRequest {
    pub identity: String,
    pub natural_language: String,
    /// Exact current-time input supplied by the caller, never inferred by the model.
    pub reference_at: TemporalInstant,
    /// Named IANA zone and exact rule-set identity supplied by the caller.
    pub reference_zone: NamedTimeZone,
    /// Deterministically resolved identities, sorted before model interpretation.
    pub participant_directory: Vec<String>,
    pub maximum_candidates: u16,
    pub maximum_results: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelativeDateWindow {
    pub start_day_offset: i16,
    pub end_day_offset: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreferredLocalWindow {
    pub start: LocalTime,
    pub end: LocalTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemporalAmbiguity {
    RelativeLanguage(String),
    TimeZoneAbbreviation(String),
    ParticipantReference(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalProposalProvenance {
    pub source: ModelResultProvenance,
    pub implementation_identity: String,
    pub request_identity: String,
    pub run_identity: String,
}

/// Inert proposal Info. It contains neither current-time truth nor effect authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalProposal {
    pub identity: String,
    pub date_window: RelativeDateWindow,
    pub duration_minutes: u16,
    pub preferred_local_window: PreferredLocalWindow,
    pub excluded_day_offsets: Vec<i16>,
    pub participant_refs: Vec<String>,
    pub unresolved_ambiguities: Vec<TemporalAmbiguity>,
    /// `None` means one meeting; a finite value is only a proposal, not a schedule.
    pub recurrence_occurrences: Option<u16>,
    /// Existing events may not be asserted by this interpretation seam.
    pub claimed_existing_event_refs: Vec<String>,
    pub provenance: TemporalProposalProvenance,
}

/// Timezone-engine output supplied to deterministic validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalResolutionTruth {
    pub reference: ZonedResolution,
    pub candidate_starts: Vec<ZonedResolution>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemporalInterpretationRefusal {
    InvalidRequest,
    InvalidModelEnvelope,
    InvalidProposal,
    MalformedDuration,
    OverBroadRecurrence,
    UnknownParticipant,
    UnresolvedAmbiguity,
    HallucinatedExistingEvent,
    ReferenceResolutionMismatch,
    MissingCivilResolution,
    AmbiguousCivilTime,
    NonexistentCivilTime,
    ArithmeticOverflow,
}

/// The portable `llm/interpret` gear boundary: correlate a bounded model result
/// with its decoded typed proposal. Resolution and effects deliberately happen later.
pub fn interpret_temporal_proposal(
    request: &TemporalInterpretationRequest,
    result: &ModelDerivedResult,
    proposal: TemporalProposal,
) -> Result<TemporalProposal, TemporalInterpretationRefusal> {
    request.validate()?;
    proposal.validate_against(request)?;
    proposal.validate_model_envelope(result)?;
    Ok(proposal)
}

impl TemporalInterpretationRequest {
    pub fn validate(&self) -> Result<(), TemporalInterpretationRefusal> {
        if !valid_identity(&self.identity)
            || self.natural_language.is_empty()
            || self.natural_language.len() > MAXIMUM_TEMPORAL_INTENT_BYTES
            || self.reference_at.validate().is_err()
            || self.reference_at.clock_basis != UNIX_UTC_CLOCK_BASIS
            || self.reference_at.uncertainty_ticks != 0
            || self.reference_zone.validate().is_err()
            || self.participant_directory.is_empty()
            || self.participant_directory.len() > MAXIMUM_TEMPORAL_PARTICIPANTS
            || !strictly_sorted_identities(&self.participant_directory)
            || self.maximum_candidates == 0
            || self.maximum_candidates > MAXIMUM_TEMPORAL_CANDIDATES
            || self.maximum_results == 0
            || self.maximum_results > self.maximum_candidates
        {
            return Err(TemporalInterpretationRefusal::InvalidRequest);
        }
        Ok(())
    }
}

impl TemporalProposal {
    fn validate_model_envelope(
        &self,
        result: &ModelDerivedResult,
    ) -> Result<(), TemporalInterpretationRefusal> {
        let contract = llm_contract(LLM_INTERPRET_KIND)
            .ok_or(TemporalInterpretationRefusal::InvalidModelEnvelope)?;
        result.validate(&contract).map_err(map_model_invalidity)?;
        if result.disposition != ModelResultDisposition::Produced
            || self.provenance.source != ModelResultProvenance::ModelDerived
            || self.provenance.implementation_identity != result.implementation_identity
            || self.provenance.request_identity != result.request_identity
            || self.provenance.run_identity != result.run_identity
            || result.payload != self.canonical_semantic_payload()
        {
            return Err(TemporalInterpretationRefusal::InvalidModelEnvelope);
        }
        Ok(())
    }

    /// Canonical provider-neutral bytes carried by the `llm/interpret` result payload.
    pub fn canonical_semantic_payload(&self) -> Vec<u8> {
        let mut encoded = Vec::new();
        push_string(&mut encoded, &self.identity);
        encoded.extend_from_slice(&self.date_window.start_day_offset.to_be_bytes());
        encoded.extend_from_slice(&self.date_window.end_day_offset.to_be_bytes());
        encoded.extend_from_slice(&self.duration_minutes.to_be_bytes());
        push_time(&mut encoded, self.preferred_local_window.start);
        push_time(&mut encoded, self.preferred_local_window.end);
        push_len(&mut encoded, self.excluded_day_offsets.len());
        for offset in &self.excluded_day_offsets {
            encoded.extend_from_slice(&offset.to_be_bytes());
        }
        push_len(&mut encoded, self.participant_refs.len());
        for participant in &self.participant_refs {
            push_string(&mut encoded, participant);
        }
        push_len(&mut encoded, self.unresolved_ambiguities.len());
        for ambiguity in &self.unresolved_ambiguities {
            let (tag, value) = match ambiguity {
                TemporalAmbiguity::RelativeLanguage(value) => (0, value),
                TemporalAmbiguity::TimeZoneAbbreviation(value) => (1, value),
                TemporalAmbiguity::ParticipantReference(value) => (2, value),
            };
            encoded.push(tag);
            push_string(&mut encoded, value);
        }
        match self.recurrence_occurrences {
            Some(value) => {
                encoded.push(1);
                encoded.extend_from_slice(&value.to_be_bytes());
            }
            None => encoded.push(0),
        }
        push_len(&mut encoded, self.claimed_existing_event_refs.len());
        for event in &self.claimed_existing_event_refs {
            push_string(&mut encoded, event);
        }
        encoded
    }

    pub fn resolve(
        &self,
        request: &TemporalInterpretationRequest,
        truth: &TemporalResolutionTruth,
    ) -> Result<MeetingProposalRequest, TemporalInterpretationRefusal> {
        request.validate()?;
        self.validate_against(request)?;
        let reference_local = validate_reference(request, &truth.reference)?;
        if truth.candidate_starts.len() > usize::from(request.maximum_candidates) {
            return Err(TemporalInterpretationRefusal::InvalidProposal);
        }

        let mut candidates = Vec::with_capacity(usize::from(request.maximum_candidates));
        for offset in self.date_window.start_day_offset..=self.date_window.end_day_offset {
            if self.excluded_day_offsets.binary_search(&offset).is_ok() {
                continue;
            }
            if candidates.len() == usize::from(request.maximum_candidates) {
                break;
            }
            let date = add_days(reference_local.date, offset)?;
            let local = LocalDateTime::new(date, self.preferred_local_window.start);
            let resolution = truth
                .candidate_starts
                .get(candidates.len())
                .ok_or(TemporalInterpretationRefusal::MissingCivilResolution)?;
            if resolution_local(resolution) != local {
                return Err(TemporalInterpretationRefusal::MissingCivilResolution);
            }
            let start = unique_resolution(resolution, &request.reference_zone)?;
            let end = add_minutes(&start, self.duration_minutes)?;
            candidates.push(MeetingCandidate {
                identity: format!("{}/candidate/{offset}", self.identity),
                interval: TemporalWindow::new(
                    start,
                    TemporalBoundary::Inclusive,
                    end,
                    TemporalBoundary::Exclusive,
                )
                .map_err(|_| TemporalInterpretationRefusal::InvalidProposal)?,
                rationale: String::from("model-proposed preference; deterministically resolved"),
            });
        }
        if candidates.is_empty() {
            return Err(TemporalInterpretationRefusal::InvalidProposal);
        }
        if truth.candidate_starts.len() != candidates.len() {
            return Err(TemporalInterpretationRefusal::InvalidProposal);
        }
        Ok(MeetingProposalRequest {
            identity: self.identity.clone(),
            reference_at: request.reference_at.clone(),
            participant_identities: self.participant_refs.clone(),
            candidates,
            maximum_results: request.maximum_results,
        })
    }

    fn validate_against(
        &self,
        request: &TemporalInterpretationRequest,
    ) -> Result<(), TemporalInterpretationRefusal> {
        if !valid_identity(&self.identity)
            || self.date_window.start_day_offset < -MAXIMUM_RELATIVE_DAYS
            || self.date_window.end_day_offset > MAXIMUM_RELATIVE_DAYS
            || self.date_window.start_day_offset > self.date_window.end_day_offset
            || self.preferred_local_window.start.validate().is_err()
            || self.preferred_local_window.end.validate().is_err()
            || local_time_nanos(self.preferred_local_window.start)
                >= local_time_nanos(self.preferred_local_window.end)
            || self.excluded_day_offsets.len() > MAXIMUM_TEMPORAL_EXCLUSIONS
            || self
                .excluded_day_offsets
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.participant_refs.is_empty()
            || self.participant_refs.len() > MAXIMUM_TEMPORAL_PARTICIPANTS
            || !strictly_sorted_identities(&self.participant_refs)
            || self.unresolved_ambiguities.len() > MAXIMUM_TEMPORAL_AMBIGUITIES
            || self
                .unresolved_ambiguities
                .iter()
                .any(|value| !ambiguity_valid(value))
            || !valid_identity(&self.provenance.implementation_identity)
            || !valid_identity(&self.provenance.request_identity)
            || !valid_identity(&self.provenance.run_identity)
        {
            return Err(TemporalInterpretationRefusal::InvalidProposal);
        }
        let preferred_duration_nanos = local_time_nanos(self.preferred_local_window.end)
            .checked_sub(local_time_nanos(self.preferred_local_window.start));
        if self.duration_minutes == 0
            || self.duration_minutes > MAXIMUM_DURATION_MINUTES
            || preferred_duration_nanos.is_none_or(|available| {
                u64::from(self.duration_minutes) * 60_000_000_000 > available
            })
        {
            return Err(TemporalInterpretationRefusal::MalformedDuration);
        }
        if self
            .recurrence_occurrences
            .is_some_and(|count| count == 0 || count > MAXIMUM_RECURRENCE_OCCURRENCES)
        {
            return Err(TemporalInterpretationRefusal::OverBroadRecurrence);
        }
        if !self.claimed_existing_event_refs.is_empty() {
            return Err(TemporalInterpretationRefusal::HallucinatedExistingEvent);
        }
        if !self.unresolved_ambiguities.is_empty() {
            return Err(TemporalInterpretationRefusal::UnresolvedAmbiguity);
        }
        if self.participant_refs.iter().any(|participant| {
            request
                .participant_directory
                .binary_search(participant)
                .is_err()
        }) {
            return Err(TemporalInterpretationRefusal::UnknownParticipant);
        }
        Ok(())
    }
}

fn validate_reference(
    request: &TemporalInterpretationRequest,
    resolution: &ZonedResolution,
) -> Result<LocalDateTime, TemporalInterpretationRefusal> {
    let ZonedResolution::Unique {
        local,
        zone,
        instant,
    } = resolution
    else {
        return Err(TemporalInterpretationRefusal::ReferenceResolutionMismatch);
    };
    resolution
        .validate()
        .map_err(|_| TemporalInterpretationRefusal::ReferenceResolutionMismatch)?;
    if zone != &request.reference_zone || instant != &request.reference_at {
        return Err(TemporalInterpretationRefusal::ReferenceResolutionMismatch);
    }
    Ok(*local)
}

fn unique_resolution(
    resolution: &ZonedResolution,
    expected_zone: &NamedTimeZone,
) -> Result<TemporalInstant, TemporalInterpretationRefusal> {
    resolution
        .validate()
        .map_err(|_| TemporalInterpretationRefusal::InvalidProposal)?;
    match resolution {
        ZonedResolution::Unique { zone, instant, .. } if zone == expected_zone => {
            Ok(instant.clone())
        }
        ZonedResolution::Unique { .. } => {
            Err(TemporalInterpretationRefusal::ReferenceResolutionMismatch)
        }
        ZonedResolution::Ambiguous { .. } => Err(TemporalInterpretationRefusal::AmbiguousCivilTime),
        ZonedResolution::Nonexistent { .. } => {
            Err(TemporalInterpretationRefusal::NonexistentCivilTime)
        }
    }
}

fn resolution_local(resolution: &ZonedResolution) -> LocalDateTime {
    match resolution {
        ZonedResolution::Unique { local, .. }
        | ZonedResolution::Ambiguous { local, .. }
        | ZonedResolution::Nonexistent { local, .. } => *local,
    }
}

fn add_minutes(
    instant: &TemporalInstant,
    minutes: u16,
) -> Result<TemporalInstant, TemporalInterpretationRefusal> {
    let ticks_per_minute = match instant.scale {
        TemporalScale::Seconds => 60,
        TemporalScale::Milliseconds => 60_000,
        TemporalScale::Microseconds => 60_000_000,
        TemporalScale::Nanoseconds => 60_000_000_000,
    };
    let delta = u64::from(minutes)
        .checked_mul(ticks_per_minute)
        .ok_or(TemporalInterpretationRefusal::ArithmeticOverflow)?;
    let mut end = instant.clone();
    end.ticks = end
        .ticks
        .checked_add(delta)
        .ok_or(TemporalInterpretationRefusal::ArithmeticOverflow)?;
    Ok(end)
}

fn add_days(date: LocalDate, offset: i16) -> Result<LocalDate, TemporalInterpretationRefusal> {
    let mut value = date;
    if offset >= 0 {
        for _ in 0..offset {
            value = next_date(value)?;
        }
    } else {
        for _ in offset..0 {
            value = previous_date(value)?;
        }
    }
    Ok(value)
}

fn next_date(date: LocalDate) -> Result<LocalDate, TemporalInterpretationRefusal> {
    let (year, month, day) = (date.year(), date.month(), date.day());
    if let Ok(next) = LocalDate::new(year, month, day.saturating_add(1)) {
        return Ok(next);
    }
    if month < 12 {
        LocalDate::new(year, month + 1, 1)
    } else {
        LocalDate::new(
            year.checked_add(1)
                .ok_or(TemporalInterpretationRefusal::ArithmeticOverflow)?,
            1,
            1,
        )
    }
    .map_err(|_| TemporalInterpretationRefusal::ArithmeticOverflow)
}

fn previous_date(date: LocalDate) -> Result<LocalDate, TemporalInterpretationRefusal> {
    let (year, month, day) = (date.year(), date.month(), date.day());
    if day > 1 {
        return LocalDate::new(year, month, day - 1)
            .map_err(|_| TemporalInterpretationRefusal::ArithmeticOverflow);
    }
    let (previous_year, previous_month) = if month > 1 {
        (year, month - 1)
    } else {
        (
            year.checked_sub(1)
                .ok_or(TemporalInterpretationRefusal::ArithmeticOverflow)?,
            12,
        )
    };
    for candidate in (28..=31).rev() {
        if let Ok(value) = LocalDate::new(previous_year, previous_month, candidate) {
            return Ok(value);
        }
    }
    Err(TemporalInterpretationRefusal::ArithmeticOverflow)
}

fn local_time_nanos(value: LocalTime) -> u64 {
    (u64::from(value.hour()) * 3_600 + u64::from(value.minute()) * 60 + u64::from(value.second()))
        * 1_000_000_000
        + u64::from(value.nanosecond())
}

fn valid_identity(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAXIMUM_TEMPORAL_IDENTITY_BYTES
}

fn strictly_sorted_identities(values: &[String]) -> bool {
    values.iter().all(|value| valid_identity(value))
        && values.windows(2).all(|pair| pair[0] < pair[1])
}

fn ambiguity_valid(value: &TemporalAmbiguity) -> bool {
    let text = match value {
        TemporalAmbiguity::RelativeLanguage(value)
        | TemporalAmbiguity::TimeZoneAbbreviation(value)
        | TemporalAmbiguity::ParticipantReference(value) => value,
    };
    !text.is_empty() && text.len() <= MAXIMUM_TEMPORAL_INTENT_BYTES
}

fn push_len(encoded: &mut Vec<u8>, value: usize) {
    encoded.extend_from_slice(&u16::try_from(value).unwrap_or(u16::MAX).to_be_bytes());
}

fn push_string(encoded: &mut Vec<u8>, value: &str) {
    encoded.extend_from_slice(&u32::try_from(value.len()).unwrap_or(u32::MAX).to_be_bytes());
    encoded.extend_from_slice(value.as_bytes());
}

fn push_time(encoded: &mut Vec<u8>, value: LocalTime) {
    encoded.extend_from_slice(&[value.hour(), value.minute(), value.second()]);
    encoded.extend_from_slice(&value.nanosecond().to_be_bytes());
}

fn map_model_invalidity(_: ModelResultInvalidity) -> TemporalInterpretationRefusal {
    TemporalInterpretationRefusal::InvalidModelEnvelope
}
