use conduit_core::{
    AvailabilityBasis, AvailabilityInterval, AvailabilityState, CalendarEvent, CalendarEventTime,
    CalendarRefusal, InvitationEvidence, InvitationState, LocalDate, LocalDateTime, LocalTime,
    MeetingCandidate, MeetingProposalRefusal, MeetingProposalRequest, NamedTimeZone, Participant,
    ParticipantAvailability, ParticipantRole, TemporalBoundary, TemporalInstant, TemporalScale,
    TemporalWindow, TimedCalendarSpan,
};

fn instant(ticks: u64) -> TemporalInstant {
    TemporalInstant {
        ticks,
        scale: TemporalScale::Seconds,
        clock_basis: "unix-utc".into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    }
}

fn window(start: u64, end: u64) -> TemporalWindow {
    TemporalWindow::new(
        instant(start),
        TemporalBoundary::Inclusive,
        instant(end),
        TemporalBoundary::Exclusive,
    )
    .unwrap()
}

fn zone(name: &str) -> NamedTimeZone {
    NamedTimeZone::new(name.into(), "tzdb/2026a".into()).unwrap()
}

fn availability(
    participant: &str,
    participant_zone: &str,
    states: &[AvailabilityState],
) -> ParticipantAvailability {
    ParticipantAvailability {
        participant_identity: participant.into(),
        zone: zone(participant_zone),
        basis: AvailabilityBasis {
            identity: format!("free-busy/{participant}/revision-7"),
            observed_at: instant(90),
            usable_until: instant(120),
        },
        intervals: states
            .iter()
            .enumerate()
            .map(|(index, state)| AvailabilityInterval {
                participant_identity: participant.into(),
                interval: window(200 + index as u64 * 100, 300 + index as u64 * 100),
                state: *state,
            })
            .collect(),
    }
}

fn request() -> MeetingProposalRequest {
    MeetingProposalRequest {
        identity: "meeting-proposal/cross-zone".into(),
        reference_at: instant(100),
        participant_identities: vec!["participant/alex".into(), "participant/sam".into()],
        candidates: (0..5)
            .map(|index| MeetingCandidate {
                identity: format!("candidate/{}", index + 1),
                interval: window(220 + index * 100, 250 + index * 100),
                rationale: "inside both participants' supplied working window".into(),
            })
            .collect(),
        maximum_results: 3,
    }
}

#[test]
fn finite_cross_zone_free_busy_yields_exactly_three_inert_candidates() {
    let alex = availability(
        "participant/alex",
        "America/Los_Angeles",
        &[
            AvailabilityState::Free,
            AvailabilityState::Free,
            AvailabilityState::Free,
            AvailabilityState::Free,
            AvailabilityState::Free,
        ],
    );
    let sam = availability(
        "participant/sam",
        "Europe/London",
        &[
            AvailabilityState::Free,
            AvailabilityState::Busy,
            AvailabilityState::Tentative,
            AvailabilityState::Free,
            AvailabilityState::Unavailable,
        ],
    );

    let proposal = request().propose(&[alex, sam]).unwrap();
    assert_eq!(proposal.candidates.len(), 3);
    assert_eq!(proposal.candidates[0].candidate_identity, "candidate/1");
    assert_eq!(proposal.candidates[1].candidate_identity, "candidate/3");
    assert_eq!(proposal.candidates[2].candidate_identity, "candidate/4");
    assert_eq!(
        proposal.candidates[1].tentative_participants,
        ["participant/sam"]
    );
    assert_eq!(proposal.rejected.len(), 2);
    assert_eq!(
        proposal.availability_basis_identities,
        [
            "free-busy/participant/alex/revision-7",
            "free-busy/participant/sam/revision-7"
        ]
    );
}

#[test]
fn stale_or_missing_availability_and_no_common_slot_refuse_distinctly() {
    let alex = availability(
        "participant/alex",
        "America/Los_Angeles",
        &[AvailabilityState::Busy; 5],
    );
    let mut sam = availability(
        "participant/sam",
        "Europe/London",
        &[AvailabilityState::Free; 5],
    );
    assert_eq!(
        request().propose(core::slice::from_ref(&alex)),
        Err(MeetingProposalRefusal::MissingParticipant)
    );
    assert_eq!(
        request().propose(&[alex.clone(), sam.clone()]),
        Err(MeetingProposalRefusal::NoCommonAvailability)
    );
    sam.basis.usable_until = instant(99);
    assert_eq!(
        request().propose(&[alex, sam]),
        Err(MeetingProposalRefusal::StaleAvailability)
    );
}

#[test]
fn timed_all_day_and_invitation_evidence_remain_mechanically_distinct() {
    let participant = Participant {
        identity: "participant/alex".into(),
        contact_reference: Some("contact/alex".into()),
        role: ParticipantRole::Required,
        invitation: InvitationEvidence::Observed {
            state: InvitationState::Tentative,
            observed_at: instant(100),
            source_identity: "calendar-observation/revision-7".into(),
        },
    };
    let timed = CalendarEventTime::Timed(TimedCalendarSpan {
        local_start: LocalDateTime::new(
            LocalDate::new(2026, 11, 1).unwrap(),
            LocalTime::new(1, 30, 0, 0).unwrap(),
        ),
        local_end: LocalDateTime::new(
            LocalDate::new(2026, 11, 1).unwrap(),
            LocalTime::new(2, 0, 0, 0).unwrap(),
        ),
        zone: zone("America/Los_Angeles"),
        instant: window(1_793_515_800, 1_793_517_600),
    });
    let all_day = CalendarEventTime::AllDay {
        start: LocalDate::new(2026, 11, 1).unwrap(),
        end_exclusive: LocalDate::new(2026, 11, 2).unwrap(),
    };
    assert_ne!(timed, all_day);

    let event = CalendarEvent {
        identity: "event/dst-boundary".into(),
        title: "Cross-zone meeting".into(),
        description: String::new(),
        location: "room/portable".into(),
        time: timed,
        participants: vec![participant],
        recurrence: None,
        reminders: vec![],
    };
    assert_eq!(event.validate(), Ok(()));
    assert!(matches!(
        event.participants[0].invitation,
        InvitationEvidence::Observed {
            state: InvitationState::Tentative,
            ..
        }
    ));

    let invalid_all_day = CalendarEventTime::AllDay {
        start: LocalDate::new(2026, 11, 2).unwrap(),
        end_exclusive: LocalDate::new(2026, 11, 1).unwrap(),
    };
    assert_eq!(
        invalid_all_day.validate(),
        Err(CalendarRefusal::InvalidTime)
    );
}

#[test]
fn overlapping_availability_is_rejected_instead_of_silently_ranked() {
    let mut current = availability(
        "participant/alex",
        "America/Los_Angeles",
        &[AvailabilityState::Free; 5],
    );
    current.intervals[1].interval = window(250, 400);
    assert_eq!(
        current.validate_at(&instant(100)),
        Err(CalendarRefusal::InvalidAvailability)
    );
}
