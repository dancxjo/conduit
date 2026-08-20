use conduit_ai::{ModelEffectProposal, ModelFollowUpTimingProposal, ModelResultProvenance};
use conduit_core::{
    kind_id, BoundedResourceRef, CalendarEvent, CalendarEventTime, CivilTrigger,
    ClockChangeBehavior, KindId, LocalDate, LocalDateTime, LocalTime, MissedOccurrencePolicy,
    NamedTimeZone, PlanId, RecurrenceDefinition, RecurrenceExpansion, RecurrenceRule,
    RecurrenceWindow, ResourceClassId, ResourceExtent, ResourceLifetime, ResourceSemanticIdentity,
    ResourceVersionIdentity, ScheduledIntent, TemporalBoundary, TemporalInstant, TemporalScale,
    TemporalWindow, TimedCalendarSpan, TriggerProfile,
};
use conduit_std_catalog::{
    validate_scheduled_job, JobOutputProfile, JobRequest, JOB_EXECUTABLE_ACCESS_CLASS,
    JOB_EXECUTABLE_CONTENT_PROFILE,
};

#[test]
fn meeting_job_and_model_follow_up_share_time_without_sharing_domain_meaning() {
    let zone = NamedTimeZone::new("America/Los_Angeles".into(), "tzdb/2026b".into()).unwrap();
    let at = wall(1_000);
    let window = TemporalWindow::new(
        at.clone(),
        TemporalBoundary::Inclusive,
        wall(1_100),
        TemporalBoundary::Inclusive,
    )
    .unwrap();
    let recurrence = RecurrenceDefinition {
        identity: "recurrence/shared-follow-up".into(),
        rule: RecurrenceRule::OneShot { at: at.clone() },
        maximum_occurrences: 1,
        until: None,
        excluded_ordinals: vec![],
    };
    let occurrence = recurrence
        .expand(&RecurrenceExpansion {
            maximum_results: 1,
            window: RecurrenceWindow::Wall {
                start: at,
                end: wall(1_100),
            },
        })
        .unwrap()
        .remove(0);

    let meeting = CalendarEvent {
        identity: "event/shared-time".into(),
        title: "Cross-zone review".into(),
        description: String::new(),
        location: String::new(),
        time: CalendarEventTime::Timed(TimedCalendarSpan {
            local_start: local(9, 0),
            local_end: local(9, 30),
            zone: zone.clone(),
            instant: window.clone(),
        }),
        participants: vec![],
        recurrence: Some(recurrence),
        reminders: vec![],
    };
    meeting.validate().unwrap();

    let job = ScheduledIntent {
        identity: "scheduled/job/shared-time#0".into(),
        occurrence: occurrence.clone(),
        trigger: TriggerProfile::Civil(CivilTrigger {
            window: window.clone(),
            zone: zone.clone(),
            clock_change: ClockChangeBehavior::ReevaluateWindow,
        }),
        missed: MissedOccurrencePolicy::Expire,
        payload: job_request(),
    };
    validate_scheduled_job(&job).unwrap();

    let follow_up = ModelFollowUpTimingProposal {
        identity: "proposal/model-follow-up-time".into(),
        provenance: ModelResultProvenance::ModelDerived,
        proposed: ScheduledIntent {
            identity: "scheduled/model-follow-up#0".into(),
            occurrence,
            trigger: TriggerProfile::Civil(CivilTrigger {
                window,
                zone,
                clock_change: ClockChangeBehavior::ReevaluateWindow,
            }),
            missed: MissedOccurrencePolicy::Skip,
            payload: ModelEffectProposal {
                proposal_id: "proposal/model-follow-up-effect".into(),
                plan_id: PlanId::from("plan/candidate"),
                operation_kind: KindId::from("process/run-bounded"),
                canonical_arguments: vec![1],
                rationale: "bounded follow-up".into(),
                evidence: vec![],
            },
        },
    };
    follow_up.validate().unwrap();

    assert_eq!(
        meeting.recurrence.as_ref().unwrap().identity,
        job.occurrence.recurrence_identity
    );
    assert_eq!(
        job.occurrence.recurrence_identity,
        follow_up.proposed.occurrence.recurrence_identity
    );
    assert_eq!(job.payload.timeout_millis, 1_000);
    assert_eq!(follow_up.provenance, ModelResultProvenance::ModelDerived);
}

fn wall(ticks: u64) -> TemporalInstant {
    TemporalInstant {
        ticks,
        scale: TemporalScale::Seconds,
        clock_basis: "unix/utc@1".into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    }
}

fn local(hour: u8, minute: u8) -> LocalDateTime {
    LocalDateTime::new(
        LocalDate::new(2026, 8, 20).unwrap(),
        LocalTime::new(hour, minute, 0, 0).unwrap(),
    )
}

fn job_request() -> JobRequest {
    let digest = [7_u8; 32];
    JobRequest {
        executable: BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest(digest),
            content_profile: kind_id(JOB_EXECUTABLE_CONTENT_PROFILE),
            access_class: ResourceClassId::from(JOB_EXECUTABLE_ACCESS_CLASS),
            extent: ResourceExtent {
                bytes: 1,
                items: Some(1),
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest(digest),
                expires_at: None,
            },
        },
        arguments: vec![],
        environment: vec![],
        stdout_profile: JobOutputProfile::Utf8,
        stderr_profile: JobOutputProfile::Utf8,
        maximum_stdout_bytes: 32,
        maximum_stderr_bytes: 32,
        timeout_millis: 1_000,
    }
}
