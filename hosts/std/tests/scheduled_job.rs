#![cfg(unix)]

use conduit_core::{
    kind_id, AuthorityContractId, AuthorityGrantId, BootId, BoundedResourceRef, HostId,
    ResourceClassId, ResourceExtent, ResourceHandleId, ResourceLifetime,
    ResourceReferenceAvailability, ResourceReferenceBinding, ResourceSemanticIdentity,
    ResourceVersionIdentity,
};
use conduit_semantic_catalog::{
    ready_job_request, JobLifecycleEvent, JobOutputProfile, JobRequest, JobTerminalOutcome,
    JOB_EXECUTABLE_ACCESS_CLASS, JOB_EXECUTABLE_AUTHORITY, JOB_EXECUTABLE_CONTENT_PROFILE,
};
use conduit_std_host::hosted_job::{
    run_bounded_job, AdmittedExecutable, HostedJobRefusal, JobCancellation,
};
use conduit_time::{
    MissedOccurrencePolicy, MonotonicClockIdentity, MonotonicDuration, MonotonicInstant,
    OccurrenceInstant, RecurrenceOccurrence, ScheduledIntent, ScheduledOccurrenceDecision,
    SuspendBehavior, TemporalScale, TriggerObservation, TriggerProfile,
};
use std::path::PathBuf;

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

    let mut denied = executable(request);
    denied.binding.authority_contract = AuthorityContractId::from("authority/not-job");
    assert!(matches!(
        run_bounded_job(request, &denied, &JobCancellation::default()),
        Err(HostedJobRefusal::Resource(_))
    ));

    let report =
        run_bounded_job(request, &executable(request), &JobCancellation::default()).unwrap();
    assert_eq!(report.stdout.bytes, b"scheduled-job");
    assert!(matches!(
        report.lifecycle.last(),
        Some(JobLifecycleEvent::Terminal(
            JobTerminalOutcome::Completed { .. }
        ))
    ));
}

fn request() -> JobRequest {
    let digest = digest("/usr/bin/printf");
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
        arguments: vec!["scheduled-job".into()],
        environment: vec![],
        stdout_profile: JobOutputProfile::Utf8,
        stderr_profile: JobOutputProfile::Utf8,
        maximum_stdout_bytes: 32,
        maximum_stderr_bytes: 32,
        timeout_millis: 1_000,
    }
}

fn executable(request: &JobRequest) -> AdmittedExecutable {
    AdmittedExecutable {
        binding: ResourceReferenceBinding {
            identity: request.executable.identity,
            version: request.executable.lifetime.version,
            content_profile: request.executable.content_profile.clone(),
            access_class: request.executable.access_class.clone(),
            handle: ResourceHandleId::from("handle:/usr/bin/printf"),
            authority_contract: AuthorityContractId::from(JOB_EXECUTABLE_AUTHORITY),
            authority_grant: AuthorityGrantId::from("grant/scheduled-job"),
            maximum_bytes: 1,
            maximum_items: Some(1),
            availability: ResourceReferenceAvailability::Available,
        },
        program: PathBuf::from("/usr/bin/printf"),
    }
}

fn digest(value: &str) -> [u8; 32] {
    let mut digest = [0_u8; 32];
    for (index, byte) in value.bytes().enumerate() {
        digest[index % 32] ^= byte;
    }
    digest
}
