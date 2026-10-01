#![cfg(unix)]

use conduit_core::{
    kind_id, BootId, BoundedResourceRef, OfferGeneration, ResourceClassId, ResourceExtent,
    ResourceLifetime, ResourceReferenceAvailability, ResourceSemanticIdentity,
    ResourceVersionIdentity,
};
use conduit_semantic_catalog::{
    JobArguments, JobEnvironment, JobEnvironmentEntry, JobExecutable, JobLifecycleEvent,
    JobOutputProfile, JobRequest, JobStreamPressure, JobTerminalOutcome, JobText,
    JOB_EXECUTABLE_ACCESS_CLASS, JOB_EXECUTABLE_CONTENT_PROFILE,
};
use conduit_std_host::hosted_job::{HostedJobRefusal, JobCancellation, TrustedJobProvider};

mod job_support;

#[test]
fn std_host_executes_without_a_shell_and_keeps_environment_exact() {
    let request = request(
        "/usr/bin/env",
        vec![],
        vec![JobEnvironmentEntry::new(text("CONDUIT_JOB_FIXTURE"), text("exact")).unwrap()],
        1_024,
        1_000,
    );
    let report = run(&request, "/usr/bin/env", &JobCancellation::default());
    assert_eq!(report.lifecycle[0], JobLifecycleEvent::Started);
    assert_eq!(report.lifecycle[1], JobLifecycleEvent::Running);
    assert!(matches!(
        report.lifecycle.last(),
        Some(JobLifecycleEvent::Terminal(JobTerminalOutcome::Completed(
            ..
        )))
    ));
    assert_eq!(
        report.stdout.bytes().get().as_slice(),
        b"CONDUIT_JOB_FIXTURE=exact\n"
    );
    assert_eq!(report.stdout.pressure(), &JobStreamPressure::WithinLimit);
}

#[test]
fn stdout_is_drained_but_retained_only_to_the_declared_bound() {
    let output = "bounded-output".repeat(12);
    let request = request("/usr/bin/printf", vec![output.clone()], vec![], 37, 1_000);
    let report = run(&request, "/usr/bin/printf", &JobCancellation::default());
    assert_eq!(report.stdout.bytes().get().as_slice().len(), 37);
    assert_eq!(*report.usage.stdout_observed_bytes(), output.len() as u64);
    assert!(matches!(
        report.stdout.pressure(),
        JobStreamPressure::Truncated(..)
    ));
    assert!(matches!(
        report.lifecycle.last(),
        Some(JobLifecycleEvent::Terminal(JobTerminalOutcome::Completed(
            ..
        )))
    ));
}

#[test]
fn failure_cancellation_timeout_and_provider_loss_are_distinct() {
    let failed = request("/usr/bin/false", vec![], vec![], 0, 1_000);
    let report = run(&failed, "/usr/bin/false", &JobCancellation::default());
    assert!(matches!(
        report.lifecycle.last(),
        Some(JobLifecycleEvent::Terminal(JobTerminalOutcome::Failed(..)))
    ));

    let cancelled = request(
        "/usr/bin/printf",
        vec!["ignored".to_string()],
        vec![],
        16,
        1_000,
    );
    let cancellation = JobCancellation::default();
    cancellation.cancel();
    let report = run(&cancelled, "/usr/bin/printf", &cancellation);
    assert!(matches!(
        report.lifecycle.last(),
        Some(JobLifecycleEvent::Terminal(JobTerminalOutcome::Cancelled(
            ..
        )))
    ));

    let timeout = request("/usr/bin/sleep", vec!["1".to_string()], vec![], 0, 5);
    let report = run(&timeout, "/usr/bin/sleep", &JobCancellation::default());
    assert!(matches!(
        report.lifecycle.last(),
        Some(JobLifecycleEvent::Terminal(JobTerminalOutcome::TimedOut(
            ..
        )))
    ));

    let lost = request(
        "/usr/bin/printf",
        vec!["ignored".to_string()],
        vec![],
        16,
        1_000,
    );
    let placement = job_support::planned_job(&lost);
    let provider = job_support::provider_with_availability(
        &lost,
        &placement,
        "/usr/bin/printf",
        ResourceReferenceAvailability::Lost,
    );
    let report = provider
        .run_bounded_job(&placement, &lost, &JobCancellation::default())
        .unwrap();
    assert!(matches!(
        report.lifecycle.last(),
        Some(JobLifecycleEvent::Terminal(
            JobTerminalOutcome::ProviderLost(..)
        ))
    ));
}

#[test]
fn forged_and_stale_execution_facts_fail_at_the_effect_boundary() {
    let request = request("/usr/bin/printf", vec!["safe".into()], vec![], 16, 1_000);
    let placement = job_support::planned_job(&request);
    let provider = job_support::provider(&request, &placement, "/usr/bin/printf");

    let mut forged_resource = request.clone();
    let mut executable = forged_resource.executable().get().clone();
    executable.identity = ResourceSemanticIdentity::from_digest([9; 32]);
    forged_resource = replace_executable(&forged_resource, executable);
    assert_eq!(
        provider.run_bounded_job(&placement, &forged_resource, &JobCancellation::default()),
        Err(HostedJobRefusal::WrongPlannedResource)
    );

    let mut stale_resource = request.clone();
    let mut executable = stale_resource.executable().get().clone();
    executable.lifetime.version = ResourceVersionIdentity::from_digest([8; 32]);
    stale_resource = replace_executable(&stale_resource, executable);
    assert_eq!(
        provider.run_bounded_job(&placement, &stale_resource, &JobCancellation::default()),
        Err(HostedJobRefusal::WrongPlannedResource)
    );

    let mut stale_provider = placement.clone();
    stale_provider.boot_id = BootId::from("boot/forged");
    assert_eq!(
        provider.run_bounded_job(&stale_provider, &request, &JobCancellation::default()),
        Err(HostedJobRefusal::StalePlannedProvider)
    );
    stale_provider = placement.clone();
    stale_provider.offer_generation = OfferGeneration(8);
    assert_eq!(
        provider.run_bounded_job(&stale_provider, &request, &JobCancellation::default()),
        Err(HostedJobRefusal::StalePlannedProvider)
    );

    let mut wrong_class = placement.clone();
    wrong_class.resources[0].class_id = ResourceClassId::from("resource/not-executable");
    assert_eq!(
        provider.run_bounded_job(&wrong_class, &request, &JobCancellation::default()),
        Err(HostedJobRefusal::WrongPlannedResource)
    );

    let empty_provider = TrustedJobProvider::new(
        placement.host_id.clone(),
        placement.boot_id.clone(),
        placement.offer_generation,
        placement.capability_id.clone(),
    );
    assert_eq!(
        empty_provider.run_bounded_job(&placement, &request, &JobCancellation::default()),
        Err(HostedJobRefusal::ExecutableUnavailable),
        "knowing /usr/bin/printf without its trusted registered capability grants nothing"
    );
}

#[test]
fn nonzero_exit_and_launch_refusal_remain_distinct_observations() {
    let request = request("process/false", vec![], vec![], 0, 1_000);
    let nonzero = run(&request, "/usr/bin/false", &JobCancellation::default());
    let launch_refused = run(
        &request,
        "/definitely/not/a/conduit-executable",
        &JobCancellation::default(),
    );

    let Some(JobLifecycleEvent::Terminal(JobTerminalOutcome::Failed(nonzero))) =
        nonzero.lifecycle.last()
    else {
        panic!("nonzero process must fail terminally")
    };
    let Some(JobLifecycleEvent::Terminal(JobTerminalOutcome::Failed(launch_refused))) =
        launch_refused.lifecycle.last()
    else {
        panic!("launch refusal must fail terminally")
    };
    assert!(matches!(
        nonzero.disposition(),
        conduit_semantic_catalog::JobExitDisposition::ExitCode(1)
    ));
    assert_eq!(
        launch_refused.disposition(),
        &conduit_semantic_catalog::JobExitDisposition::Signal
    );
}

fn request(
    identity: &str,
    arguments: Vec<String>,
    environment: Vec<JobEnvironmentEntry>,
    maximum_stdout_bytes: u32,
    timeout_millis: u64,
) -> JobRequest {
    let digest = digest(identity);
    JobRequest::new(
        JobArguments::new(
            conduit_form::rust_binding::BoundedSequence::try_from_iter(
                arguments
                    .into_iter()
                    .map(|value| JobText::new(value).unwrap()),
            )
            .unwrap(),
        )
        .unwrap(),
        JobEnvironment::new(
            conduit_form::rust_binding::BoundedSequence::try_from_iter(environment).unwrap(),
        )
        .unwrap(),
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
        1_024,
        maximum_stdout_bytes,
        JobOutputProfile::Utf8,
        JobOutputProfile::Utf8,
        timeout_millis,
    )
    .unwrap()
}

fn replace_executable(request: &JobRequest, executable: BoundedResourceRef) -> JobRequest {
    JobRequest::new(
        request.arguments().clone(),
        request.environment().clone(),
        JobExecutable::new(executable).unwrap(),
        *request.maximum_stderr_bytes(),
        *request.maximum_stdout_bytes(),
        *request.stderr_profile(),
        *request.stdout_profile(),
        *request.timeout_millis(),
    )
    .unwrap()
}

fn text(value: &str) -> JobText {
    JobText::new(value.to_string()).unwrap()
}

fn run(
    request: &JobRequest,
    path: &str,
    cancellation: &JobCancellation,
) -> conduit_std_host::hosted_job::HostedJobReport {
    let placement = job_support::planned_job(request);
    job_support::provider(request, &placement, path)
        .run_bounded_job(&placement, request, cancellation)
        .unwrap()
}

fn digest(value: &str) -> [u8; 32] {
    let mut digest = [0_u8; 32];
    let length = digest.len();
    for (index, byte) in value.bytes().enumerate() {
        digest[index % length] ^= byte;
    }
    if digest == [0; 32] {
        digest[0] = 1;
    }
    digest
}
