#![cfg(unix)]

use conduit_core::{
    kind_id, AuthorityContractId, BootId, BoundedResourceRef, HostId, ResourceClassId,
    ResourceExtent, ResourceLifetime, ResourceSemanticIdentity, ResourceVersionIdentity,
};
use conduit_semantic_catalog::{
    ready_job_request, JobArguments, JobEnvironment, JobExecutable, JobLifecycleEvent,
    JobOutputProfile, JobRequest, JobTerminalOutcome, JobText, JOB_EXECUTABLE_ACCESS_CLASS,
    JOB_EXECUTABLE_CONTENT_PROFILE,
};
use conduit_std_host::hosted_job::{HostedJobRefusal, JobCancellation};
use conduit_time::{
    MissedOccurrencePolicy, MonotonicClockIdentity, MonotonicDuration, MonotonicInstant,
    OccurrenceInstant, RecurrenceOccurrence, ScheduledIntent, ScheduledOccurrenceDecision,
    SuspendBehavior, TemporalScale, TriggerObservation, TriggerProfile,
};

mod job_support;

#[test]
fn ready_elapsed_occurrence_executes_only_through_separate_job_authority() {
    let clock = MonotonicClockIdentity::new(
        HostId::from("host/scheduled-job"),
        BootId::from("boot/scheduled-job"),
        "std/monotonic@1".into(),
        TemporalScale::Milliseconds,
        1,
        0,
    )
    .unwrap();
    let opens = MonotonicInstant::new(100, clock.clone()).unwrap();
    let request = request();
    let scheduled = ScheduledIntent {
        identity: "scheduled/job/printf#0".into(),
        occurrence: RecurrenceOccurrence {
            identity: "recurrence/job/occurrence/0".into(),
            recurrence_identity: "recurrence/job".into(),
            ordinal: 0,
            at: OccurrenceInstant::Monotonic(opens.clone()),
        },
        trigger: TriggerProfile::Elapsed(
            conduit_time::elapsed_trigger_window(
                opens,
                MonotonicDuration::new(10, TemporalScale::Milliseconds),
                SuspendBehavior::ClockIncludesSuspend,
            )
            .unwrap(),
        ),
        missed: MissedOccurrencePolicy::Expire,
        payload: request,
    };
    let decision = scheduled
        .decide(
            &TriggerObservation::Elapsed {
                now: MonotonicInstant::new(102, clock).unwrap(),
                suspend_observed: false,
            },
            false,
        )
        .unwrap();
    assert_eq!(
        decision,
        ScheduledOccurrenceDecision::Ready { lateness_ticks: 2 }
    );
    let request = ready_job_request(&scheduled, decision).unwrap();

    let placement = job_support::planned_job(request);
    let provider = job_support::provider(request, &placement, "/usr/bin/printf");
    let mut denied = placement.clone();
    denied.authority[0].contract_id = AuthorityContractId::from("authority/not-job");
    assert!(matches!(
        provider.run_bounded_job(&denied, request, &JobCancellation::default()),
        Err(HostedJobRefusal::WrongPlannedAuthority)
    ));

    let report = provider
        .run_bounded_job(&placement, request, &JobCancellation::default())
        .unwrap();
    assert_eq!(report.stdout.bytes().get().as_slice(), b"scheduled-job");
    assert!(matches!(
        report.lifecycle.last(),
        Some(JobLifecycleEvent::Terminal(JobTerminalOutcome::Completed(
            ..
        )))
    ));
}

fn request() -> JobRequest {
    let digest = digest("/usr/bin/printf");
    JobRequest::new(
        JobArguments::new(
            conduit_plot::rust_binding::BoundedSequence::try_from_iter([JobText::new(
                "scheduled-job".into(),
            )
            .unwrap()])
            .unwrap(),
        )
        .unwrap(),
        JobEnvironment::new(Default::default()).unwrap(),
        JobExecutable::new(BoundedResourceRef {
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
        })
        .unwrap(),
        32,
        32,
        JobOutputProfile::Utf8,
        JobOutputProfile::Utf8,
        1_000,
    )
    .unwrap()
}

fn digest(value: &str) -> [u8; 32] {
    let mut digest = [0_u8; 32];
    for (index, byte) in value.bytes().enumerate() {
        digest[index % 32] ^= byte;
    }
    digest
}
