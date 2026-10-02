//! Deterministic bounded evaluation of supplied meeting candidates.

use alloc::vec::Vec;
use conduit_plot::rust_binding::BoundedSequence;

use crate::{
    AvailabilityState, CalendarRefusal, CandidateConflict, MeetingProposal, MeetingProposalRefusal,
    MeetingProposalRequest, ParticipantAvailability, ProposedMeetingSlot, RejectedMeetingSlot,
    TemporalRelation, TemporalWindow,
};

pub const MAXIMUM_MEETING_CANDIDATES: usize = 64;
pub const MAXIMUM_PROPOSAL_PARTICIPANTS: usize = 64;

impl MeetingProposalRequest {
    pub fn propose(
        &self,
        availability: &[ParticipantAvailability],
    ) -> Result<MeetingProposal, MeetingProposalRefusal> {
        self.validate()?;
        if availability.len() != self.participant_identities.len() {
            return Err(MeetingProposalRefusal::MissingParticipant);
        }
        for (participant, current) in self.participant_identities.iter().zip(availability) {
            if participant != &current.participant_identity {
                return Err(MeetingProposalRefusal::MissingParticipant);
            }
            current
                .validate_at(&self.reference_at)
                .map_err(map_calendar)?;
        }
        let mut accepted = Vec::with_capacity(usize::from(self.maximum_results));
        let mut rejected = Vec::with_capacity(self.candidates.len());
        for candidate in &self.candidates {
            let mut conflicts = Vec::new();
            let mut tentative = Vec::new();
            for participant in availability {
                let state = state_during(&candidate.interval, participant)?;
                match state {
                    AvailabilityState::Free => {}
                    AvailabilityState::Tentative => {
                        tentative.push(participant.participant_identity.clone())
                    }
                    AvailabilityState::Busy | AvailabilityState::Unavailable => conflicts.push(
                        CandidateConflict::new(participant.participant_identity.clone(), state)
                            .expect("validated availability owns a bounded participant identity"),
                    ),
                }
            }
            if conflicts.is_empty() && accepted.len() < usize::from(self.maximum_results) {
                accepted.push(ProposedMeetingSlot {
                    candidate_identity: candidate.identity.clone(),
                    interval: candidate.interval.clone(),
                    rationale: candidate.rationale.clone(),
                    tentative_participants: bounded(tentative),
                });
            } else if !conflicts.is_empty() {
                rejected.push(RejectedMeetingSlot {
                    candidate_identity: candidate.identity.clone(),
                    conflicts: bounded(conflicts),
                });
            }
        }
        if accepted.is_empty() {
            return Err(MeetingProposalRefusal::NoCommonAvailability);
        }
        Ok(MeetingProposal {
            identity: self.identity.clone(),
            reference_at: self.reference_at.clone(),
            availability_basis_identities: bounded(
                availability
                    .iter()
                    .map(|value| value.basis.identity.clone())
                    .collect(),
            ),
            candidates: bounded(accepted),
            rejected: bounded(rejected),
        })
    }

    fn validate(&self) -> Result<(), MeetingProposalRefusal> {
        self.reference_at
            .validate()
            .map_err(|_| MeetingProposalRefusal::InvalidRequest)?;
        if self.identity.is_empty()
            || self.participant_identities.is_empty()
            || self.participant_identities.len() > MAXIMUM_PROPOSAL_PARTICIPANTS
            || self
                .participant_identities
                .iter()
                .zip(self.participant_identities.iter().skip(1))
                .any(|(left, right)| left >= right)
            || self.candidates.is_empty()
            || self.candidates.len() > MAXIMUM_MEETING_CANDIDATES
            || self.maximum_results == 0
            || usize::from(self.maximum_results) > self.candidates.len()
        {
            return Err(MeetingProposalRefusal::InvalidRequest);
        }
        for candidate in &self.candidates {
            if candidate.identity.is_empty()
                || candidate.rationale.len() > crate::MAXIMUM_CALENDAR_TEXT_BYTES
            {
                return Err(MeetingProposalRefusal::InvalidRequest);
            }
            candidate
                .interval
                .validate()
                .map_err(|_| MeetingProposalRefusal::InvalidRequest)?;
        }
        if self
            .candidates
            .iter()
            .enumerate()
            .any(|(index, candidate)| {
                self.candidates
                    .iter()
                    .take(index)
                    .any(|earlier| earlier.identity == candidate.identity)
            })
        {
            return Err(MeetingProposalRefusal::InvalidRequest);
        }
        Ok(())
    }
}

fn bounded<T, const MAXIMUM: usize>(values: Vec<T>) -> BoundedSequence<T, MAXIMUM> {
    BoundedSequence::try_from_iter(values).expect("validated family bound")
}

fn state_during(
    candidate: &TemporalWindow,
    availability: &ParticipantAvailability,
) -> Result<AvailabilityState, MeetingProposalRefusal> {
    for interval in &availability.intervals {
        let starts = candidate
            .start()
            .relation_to(interval.interval.start())
            .map_err(|_| MeetingProposalRefusal::IncomparableTime)?;
        let ends = candidate
            .end()
            .relation_to(interval.interval.end())
            .map_err(|_| MeetingProposalRefusal::IncomparableTime)?;
        if matches!(
            starts,
            TemporalRelation::Present | TemporalRelation::Future { .. }
        ) && matches!(
            ends,
            TemporalRelation::Present | TemporalRelation::Past { .. }
        ) {
            return Ok(interval.state);
        }
        if matches!(starts, TemporalRelation::Indeterminate)
            || matches!(ends, TemporalRelation::Indeterminate)
        {
            return Err(MeetingProposalRefusal::IncomparableTime);
        }
    }
    Ok(AvailabilityState::Unavailable)
}

fn map_calendar(error: CalendarRefusal) -> MeetingProposalRefusal {
    match error {
        CalendarRefusal::StaleAvailability => MeetingProposalRefusal::StaleAvailability,
        CalendarRefusal::IncomparableTime => MeetingProposalRefusal::IncomparableTime,
        _ => MeetingProposalRefusal::InvalidAvailability,
    }
}
