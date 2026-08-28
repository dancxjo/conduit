use conduit_ai::{
    ModelEffectProposal, ModelFollowUpTimingProposal, ModelResultProvenance,
    ProposalDecisionOutcome, ProposalGate, ProposalRefusal,
};
use conduit_core::{
    AuthorityBinding, AuthorityContractId, AuthorityGrantId, BootId, CapabilityId, HostId,
    HostOperationContractId, KindId, PlanId, SignId,
};
use conduit_presentation::present_timed_calendar_event;
use conduit_semantic_catalog::{
    CALENDAR_CREATE_KIND, REMINDER_DELIVERY_AUTHORITY, REMINDER_DELIVER_KIND,
};
use conduit_std_host::hosted_calendar::{
    google_calendar_authority_grant, google_calendar_offers, GoogleBearerToken,
    GoogleCalendarClient, GoogleCalendarExchange, GoogleCalendarRefusal, GoogleCalendarResource,
    GoogleCalendarResponse, GoogleCalendarTransport, GoogleEventWriteRequest, GoogleWireEvent,
    GoogleWireEventTime,
};
use conduit_std_host::hosted_reminder::{
    deliver_ready_reminder, HostedReminderAdapter, ReminderAdapterError, ReminderDeliveryRefusal,
};
use conduit_std_offers::REMINDER_DELIVER_OPERATION;
use conduit_time::{
    elapsed_trigger_window, AvailabilityBasis, AvailabilityInterval, AvailabilityState,
    CalendarEvent, CalendarEventTime, CivilTrigger, ClockChangeBehavior, InvitationEvidence,
    LocalDate, LocalDateTime, LocalTime, MeetingCandidate, MeetingProposalRefusal,
    MeetingProposalRequest, MissedOccurrencePolicy, MonotonicClockIdentity, MonotonicDuration,
    MonotonicInstant, NamedTimeZone, OccurrenceInstant, Participant, ParticipantAvailability,
    ParticipantRole, RecurrenceOccurrence, ReminderOccurrence, ScheduledIntent,
    ScheduledOccurrenceDecision, SuspendBehavior, TemporalBoundary, TemporalInstant, TemporalScale,
    TemporalWindow, TimedCalendarSpan, TriggerObservation, TriggerProfile, ZonedResolution,
    UNIX_UTC_CLOCK_BASIS,
};

#[test]
fn human_machine_and_model_commitments_share_time_but_not_identity_or_authority() {
    let fixture = meeting_fixture();
    let proposal = fixture.request.propose(&fixture.availability).unwrap();
    assert_eq!(proposal.candidates.len(), 3);
    assert_eq!(proposal.availability_basis_identities.len(), 2);

    // Human approval selects one inert candidate before any provider operation.
    let approved = proposal.candidates[1].clone();
    let event = event_from(&approved);
    let create_offer = google_calendar_offers()
        .into_iter()
        .find(|offer| offer.kind_id.as_str() == CALENDAR_CREATE_KIND)
        .unwrap();
    let grant = google_calendar_authority_grant(
        &create_offer,
        0,
        "grant/calendar/create/capstone",
        &HostId::from("host/calendar/capstone"),
        &BootId::from("boot/calendar/capstone"),
    )
    .unwrap();
    assert_eq!(
        grant.contract_id,
        create_offer.authority_requirements[0].contract_id
    );

    let mut provider = provider(vec![200]);
    let receipt = provider
        .create_event(&GoogleEventWriteRequest {
            event: wire_event(),
            send_updates: false,
        })
        .unwrap();
    assert_eq!(receipt.event_id, "provider-event-17");
    assert_eq!(receipt.revision, "provider-revision-4");

    let los_angeles = zone("America/Los_Angeles", "tzdb/2026b");
    let london = zone("Europe/London", "tzdb/2026b");
    let event_view = present_timed_calendar_event(
        &event,
        &unique(local(2026, 8, 25, 9, 30), los_angeles.clone(), wall(3_000)),
        &unique(local(2026, 8, 25, 10, 0), los_angeles, wall(4_800)),
        &unique(local(2026, 8, 25, 17, 30), london.clone(), wall(3_000)),
        &unique(local(2026, 8, 25, 18, 0), london, wall(4_800)),
    )
    .unwrap();
    assert_eq!(event_view.event_identity, event.identity);
    assert_ne!(event_view.event_start.local, event_view.viewer_start.local);
    assert_eq!(event_view.exact_start, approved.interval.start().clone());

    let reminder = meeting_reminder(&event.identity);
    let ready = reminder
        .decide(
            &TriggerObservation::Civil {
                now: wall(4_910),
                clock_change_observed: false,
            },
            false,
        )
        .unwrap();
    let mut delivery = RecordingReminder::default();
    assert_eq!(
        deliver_ready_reminder(&reminder, ready, None, &mut delivery),
        Err(ReminderDeliveryRefusal::MissingAuthority)
    );
    let reminder_receipt =
        deliver_ready_reminder(&reminder, ready, Some(&reminder_grant()), &mut delivery).unwrap();
    assert_eq!(reminder_receipt.event_identity, event.identity);
    assert_ne!(
        reminder_receipt.reminder_occurrence_identity,
        event.identity
    );

    let model_follow_up = model_follow_up();
    model_follow_up.validate().unwrap();
    let mut gate = ProposalGate::new(None, 1).unwrap();
    let denied = gate
        .submit(model_follow_up.effect_proposal().clone())
        .unwrap();
    assert_eq!(
        denied.decision.outcome,
        ProposalDecisionOutcome::Refused(ProposalRefusal::MissingAuthority)
    );
    assert!(denied.request.is_none());

    assert_ne!(event.identity, reminder.identity);
    assert_ne!(reminder.identity, model_follow_up.identity);
    assert_eq!(
        model_follow_up.provenance,
        ModelResultProvenance::ModelDerived
    );
    assert_eq!(
        delivery.identities,
        ["reminder/event/capstone/occurrence/0"]
    );
}

#[test]
fn capstone_failures_remain_distinct_across_time_and_provider_boundaries() {
    let mut fixture = meeting_fixture();
    for participant in &mut fixture.availability {
        for interval in &mut participant.intervals {
            interval.state = AvailabilityState::Busy;
        }
    }
    assert_eq!(
        fixture.request.propose(&fixture.availability),
        Err(MeetingProposalRefusal::NoCommonAvailability)
    );
    fixture = meeting_fixture();
    fixture.availability[0].basis.usable_until = wall(999);
    assert_eq!(
        fixture.request.propose(&fixture.availability),
        Err(MeetingProposalRefusal::StaleAvailability)
    );

    let mut denied = provider(vec![403]);
    assert_eq!(
        denied.create_event(&GoogleEventWriteRequest {
            event: wire_event(),
            send_updates: false,
        }),
        Err(GoogleCalendarRefusal::AuthorityDenied)
    );

    let elapsed = elapsed_follow_up();
    let same_clock = match &elapsed.trigger {
        TriggerProfile::Elapsed(trigger) => trigger.opens_at.clock().clone(),
        TriggerProfile::Civil(_) => unreachable!(),
    };
    assert_eq!(
        elapsed.decide(
            &TriggerObservation::Elapsed {
                now: MonotonicInstant::new(110, same_clock.clone()).unwrap(),
                suspend_observed: true,
            },
            false,
        ),
        Ok(ScheduledOccurrenceDecision::Suspended)
    );
    let rebooted = MonotonicClockIdentity::new(
        HostId::from("host/machine"),
        BootId::from("boot/replacement"),
        "std/monotonic@1".into(),
        TemporalScale::Milliseconds,
        1,
        0,
    )
    .unwrap();
    assert_eq!(
        elapsed.decide(
            &TriggerObservation::Elapsed {
                now: MonotonicInstant::new(110, rebooted).unwrap(),
                suspend_observed: false,
            },
            false,
        ),
        Ok(ScheduledOccurrenceDecision::Rebooted)
    );
    assert_eq!(
        elapsed.decide(
            &TriggerObservation::Elapsed {
                now: MonotonicInstant::new(110, same_clock).unwrap(),
                suspend_observed: false,
            },
            false,
        ),
        Ok(ScheduledOccurrenceDecision::Ready { lateness_ticks: 10 })
    );

    let civil = meeting_reminder("event/capstone");
    assert_eq!(
        civil.decide(
            &TriggerObservation::Civil {
                now: wall(4_010),
                clock_change_observed: true,
            },
            false,
        ),
        Ok(ScheduledOccurrenceDecision::ClockChanged)
    );
}

struct MeetingFixture {
    request: MeetingProposalRequest,
    availability: Vec<ParticipantAvailability>,
}

fn meeting_fixture() -> MeetingFixture {
    let candidates = [1_100, 3_000, 5_000]
        .into_iter()
        .enumerate()
        .map(|(index, start)| MeetingCandidate {
            identity: format!("candidate/capstone/{index}"),
            interval: window(start, start + 1_800),
            rationale: "finite cross-zone candidate".into(),
        })
        .collect::<Vec<_>>();
    let participant = |identity: &str, zone_name: &str| ParticipantAvailability {
        participant_identity: identity.into(),
        zone: zone(zone_name, "tzdb/2026b"),
        basis: AvailabilityBasis {
            identity: format!("free-busy/{identity}/17"),
            observed_at: wall(990),
            usable_until: wall(1_010),
        },
        intervals: candidates
            .iter()
            .map(|candidate| AvailabilityInterval {
                participant_identity: identity.into(),
                interval: candidate.interval.clone(),
                state: AvailabilityState::Free,
            })
            .collect(),
    };
    let availability = vec![
        participant("person/alex", "America/Los_Angeles"),
        participant("person/bob", "Europe/London"),
    ];
    MeetingFixture {
        request: MeetingProposalRequest {
            identity: "proposal/cross-zone/capstone".into(),
            reference_at: wall(1_000),
            participant_identities: vec!["person/alex".into(), "person/bob".into()],
            candidates,
            maximum_results: 3,
        },
        availability,
    }
}

fn event_from(approved: &conduit_time::ProposedMeetingSlot) -> CalendarEvent {
    CalendarEvent {
        identity: "event/cross-zone/capstone".into(),
        title: "Cross-zone capstone".into(),
        description: String::new(),
        location: String::new(),
        time: CalendarEventTime::Timed(TimedCalendarSpan {
            local_start: local(2026, 8, 25, 9, 30),
            local_end: local(2026, 8, 25, 10, 0),
            zone: zone("America/Los_Angeles", "tzdb/2026b"),
            instant: approved.interval.clone(),
        }),
        participants: vec![
            Participant {
                identity: "person/alex".into(),
                contact_reference: Some("calendar/alex".into()),
                role: ParticipantRole::Organizer,
                invitation: InvitationEvidence::Unknown,
            },
            Participant {
                identity: "person/bob".into(),
                contact_reference: Some("calendar/bob".into()),
                role: ParticipantRole::Required,
                invitation: InvitationEvidence::Unknown,
            },
        ],
        recurrence: None,
        reminders: vec![],
    }
}

fn meeting_reminder(event_identity: &str) -> ScheduledIntent<ReminderOccurrence> {
    ScheduledIntent {
        identity: "scheduled/reminder/capstone#0".into(),
        occurrence: RecurrenceOccurrence {
            identity: "recurrence/reminder/capstone/occurrence/0".into(),
            recurrence_identity: "recurrence/reminder/capstone".into(),
            ordinal: 0,
            at: OccurrenceInstant::Wall(wall(4_900)),
        },
        trigger: TriggerProfile::Civil(CivilTrigger {
            window: window(4_900, 5_000),
            zone: zone("America/Los_Angeles", "tzdb/2026b"),
            clock_change: ClockChangeBehavior::RefuseAfterChange,
        }),
        missed: MissedOccurrencePolicy::Skip,
        payload: ReminderOccurrence {
            identity: "reminder/event/capstone/occurrence/0".into(),
            reminder_identity: "reminder/event/capstone".into(),
            event_identity: event_identity.into(),
            delivery_kind: "notification/local".into(),
        },
    }
}

fn elapsed_follow_up() -> ScheduledIntent<&'static str> {
    let clock = MonotonicClockIdentity::new(
        HostId::from("host/machine"),
        BootId::from("boot/original"),
        "std/monotonic@1".into(),
        TemporalScale::Milliseconds,
        1,
        0,
    )
    .unwrap();
    let opens = MonotonicInstant::new(100, clock).unwrap();
    ScheduledIntent {
        identity: "scheduled/machine/capstone#0".into(),
        occurrence: RecurrenceOccurrence {
            identity: "recurrence/machine/capstone/occurrence/0".into(),
            recurrence_identity: "recurrence/machine/capstone".into(),
            ordinal: 0,
            at: OccurrenceInstant::Monotonic(opens.clone()),
        },
        trigger: TriggerProfile::Elapsed(
            elapsed_trigger_window(
                opens,
                MonotonicDuration::new(20, TemporalScale::Milliseconds),
                SuspendBehavior::RefuseAfterSuspend,
            )
            .unwrap(),
        ),
        missed: MissedOccurrencePolicy::Expire,
        payload: "machine-payload",
    }
}

fn model_follow_up() -> ModelFollowUpTimingProposal {
    let schedule = elapsed_follow_up();
    ModelFollowUpTimingProposal {
        identity: "proposal/model/capstone".into(),
        provenance: ModelResultProvenance::ModelDerived,
        proposed: ScheduledIntent {
            identity: "scheduled/model/capstone#0".into(),
            occurrence: schedule.occurrence,
            trigger: schedule.trigger,
            missed: MissedOccurrencePolicy::Skip,
            payload: ModelEffectProposal {
                proposal_id: "proposal/model/capstone/effect".into(),
                plan_id: PlanId::from("plan/model/capstone"),
                operation_kind: KindId::from(REMINDER_DELIVER_KIND),
                canonical_arguments: vec![1],
                rationale: "follow up after the meeting".into(),
                evidence: vec![SignId::from("sign/meeting/created")],
            },
        },
    }
}

fn reminder_grant() -> AuthorityBinding {
    AuthorityBinding {
        grant_id: AuthorityGrantId::from("grant/reminder/capstone"),
        contract_id: AuthorityContractId::from(REMINDER_DELIVERY_AUTHORITY),
        host_operation_contract_id: HostOperationContractId::from(REMINDER_DELIVER_OPERATION),
        subject_kind: KindId::from(REMINDER_DELIVER_KIND),
        host_id: HostId::from("host/reminder/capstone"),
        boot_id: BootId::from("boot/reminder/capstone"),
        capability_id: CapabilityId::from("capability/reminder/capstone"),
    }
}

#[derive(Default)]
struct RecordingReminder {
    identities: Vec<String>,
}

impl HostedReminderAdapter for RecordingReminder {
    fn deliver(&mut self, reminder: &ReminderOccurrence) -> Result<(), ReminderAdapterError> {
        self.identities.push(reminder.identity.clone());
        Ok(())
    }
}

struct CalendarTransport {
    statuses: Vec<u16>,
}

impl GoogleCalendarTransport for CalendarTransport {
    fn exchange(
        &mut self,
        _credential: &GoogleBearerToken,
        request: &GoogleCalendarExchange,
    ) -> Result<GoogleCalendarResponse, GoogleCalendarRefusal> {
        let status = self.statuses.remove(0);
        let mut event: GoogleWireEvent = serde_json::from_slice(&request.body).unwrap();
        event.id = Some("provider-event-17".into());
        event.etag = Some("provider-revision-4".into());
        Ok(GoogleCalendarResponse {
            status,
            body: serde_json::to_vec(&event).unwrap(),
            observed_unix_seconds: 1_777_000_000,
        })
    }
}

fn provider(statuses: Vec<u16>) -> GoogleCalendarClient<CalendarTransport> {
    GoogleCalendarClient::new(
        CalendarTransport { statuses },
        GoogleBearerToken::new(b"fixture-secret".to_vec()).unwrap(),
        GoogleCalendarResource {
            account_identity: "account/capstone".into(),
            calendar_id: "calendar/capstone".into(),
        },
    )
    .unwrap()
}

fn wire_event() -> GoogleWireEvent {
    GoogleWireEvent {
        id: None,
        etag: None,
        summary: "Cross-zone capstone".into(),
        description: String::new(),
        location: String::new(),
        start: GoogleWireEventTime {
            date: None,
            date_time: Some("2026-08-25T09:30:00-07:00".into()),
            time_zone: Some("America/Los_Angeles".into()),
        },
        end: GoogleWireEventTime {
            date: None,
            date_time: Some("2026-08-25T10:00:00-07:00".into()),
            time_zone: Some("America/Los_Angeles".into()),
        },
        attendees: vec![],
        recurrence: vec![],
        status: None,
    }
}

fn unique(local: LocalDateTime, zone: NamedTimeZone, instant: TemporalInstant) -> ZonedResolution {
    ZonedResolution::Unique {
        local,
        zone,
        instant,
    }
}

fn local(year: i32, month: u8, day: u8, hour: u8, minute: u8) -> LocalDateTime {
    LocalDateTime::new(
        LocalDate::new(year, month, day).unwrap(),
        LocalTime::new(hour, minute, 0, 0).unwrap(),
    )
}

fn zone(identity: &str, rule_set: &str) -> NamedTimeZone {
    NamedTimeZone::new(identity.into(), rule_set.into()).unwrap()
}

fn window(start: u64, end: u64) -> TemporalWindow {
    TemporalWindow::new(
        wall(start),
        TemporalBoundary::Inclusive,
        wall(end),
        TemporalBoundary::Exclusive,
    )
    .unwrap()
}

fn wall(ticks: u64) -> TemporalInstant {
    TemporalInstant {
        ticks,
        scale: TemporalScale::Seconds,
        clock_basis: UNIX_UTC_CLOCK_BASIS.into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    }
}
