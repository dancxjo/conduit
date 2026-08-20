#![cfg(unix)]

use conduit_core::{
    kind_id, AuthorityContractId, AuthorityGrantId, BoundedResourceRef, ResourceClassId,
    ResourceExtent, ResourceHandleId, ResourceLifetime, ResourceReferenceAvailability,
    ResourceReferenceBinding, ResourceSemanticIdentity, ResourceVersionIdentity,
};
use conduit_std_catalog::{
    JobEnvironmentEntry, JobLifecycleEvent, JobOutputProfile, JobRequest, JobStreamPressure,
    JobTerminalOutcome, JOB_EXECUTABLE_ACCESS_CLASS, JOB_EXECUTABLE_AUTHORITY,
    JOB_EXECUTABLE_CONTENT_PROFILE,
};
use conduit_std_host::hosted_job::{run_bounded_job, AdmittedExecutable, JobCancellation};
use std::path::PathBuf;

#[test]
fn std_host_executes_without_a_shell_and_keeps_environment_exact() {
    let request = request(
        "/usr/bin/env",
        vec![],
        vec![JobEnvironmentEntry {
            name: "CONDUIT_JOB_FIXTURE".to_string(),
            value: "exact".to_string(),
        }],
        1_024,
        1_000,
    );
    let report = run_bounded_job(
        &request,
        &executable(&request, "/usr/bin/env"),
        &JobCancellation::default(),
    )
    .unwrap();
    assert_eq!(report.lifecycle[0], JobLifecycleEvent::Started);
    assert_eq!(report.lifecycle[1], JobLifecycleEvent::Running);
    assert!(matches!(
        report.lifecycle.last(),
        Some(JobLifecycleEvent::Terminal(
            JobTerminalOutcome::Completed { .. }
        ))
    ));
    assert_eq!(report.stdout.bytes, b"CONDUIT_JOB_FIXTURE=exact\n");
    assert_eq!(report.stdout.pressure, JobStreamPressure::WithinLimit);
}

#[test]
fn stdout_is_drained_but_retained_only_to_the_declared_bound() {
    let request = request("/usr/bin/yes", vec!["bounded".to_string()], vec![], 37, 20);
    let report = run_bounded_job(
        &request,
        &executable(&request, "/usr/bin/yes"),
        &JobCancellation::default(),
    )
    .unwrap();
    assert_eq!(report.stdout.bytes.len(), 37);
    assert!(report.usage.stdout_observed_bytes >= 37);
    assert!(matches!(
        report.stdout.pressure,
        JobStreamPressure::Truncated { .. }
    ));
    assert!(matches!(
        report.lifecycle.last(),
        Some(JobLifecycleEvent::Terminal(
            JobTerminalOutcome::TimedOut { .. }
        ))
    ));
}

#[test]
fn failure_cancellation_timeout_and_provider_loss_are_distinct() {
    let failed = request("/usr/bin/false", vec![], vec![], 0, 1_000);
    let report = run_bounded_job(
        &failed,
        &executable(&failed, "/usr/bin/false"),
        &JobCancellation::default(),
    )
    .unwrap();
    assert!(matches!(
        report.lifecycle.last(),
        Some(JobLifecycleEvent::Terminal(
            JobTerminalOutcome::Failed { .. }
        ))
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
    let report = run_bounded_job(
        &cancelled,
        &executable(&cancelled, "/usr/bin/printf"),
        &cancellation,
    )
    .unwrap();
    assert!(matches!(
        report.lifecycle.last(),
        Some(JobLifecycleEvent::Terminal(
            JobTerminalOutcome::Cancelled { .. }
        ))
    ));

    let timeout = request("/usr/bin/sleep", vec!["1".to_string()], vec![], 0, 5);
    let report = run_bounded_job(
        &timeout,
        &executable(&timeout, "/usr/bin/sleep"),
        &JobCancellation::default(),
    )
    .unwrap();
    assert!(matches!(
        report.lifecycle.last(),
        Some(JobLifecycleEvent::Terminal(
            JobTerminalOutcome::TimedOut { .. }
        ))
    ));

    let lost = request(
        "/usr/bin/printf",
        vec!["ignored".to_string()],
        vec![],
        16,
        1_000,
    );
    let mut binding = executable(&lost, "/usr/bin/printf");
    binding.binding.availability = ResourceReferenceAvailability::Lost;
    let report = run_bounded_job(&lost, &binding, &JobCancellation::default()).unwrap();
    assert!(matches!(
        report.lifecycle.last(),
        Some(JobLifecycleEvent::Terminal(
            JobTerminalOutcome::ProviderLost { .. }
        ))
    ));
}

fn request(
    identity: &str,
    arguments: Vec<String>,
    environment: Vec<JobEnvironmentEntry>,
    maximum_stdout_bytes: u32,
    timeout_millis: u64,
) -> JobRequest {
    let digest = digest(identity);
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
        arguments,
        environment,
        stdout_profile: JobOutputProfile::Utf8,
        stderr_profile: JobOutputProfile::Utf8,
        maximum_stdout_bytes,
        maximum_stderr_bytes: 1_024,
        timeout_millis,
    }
}

fn executable(request: &JobRequest, path: &str) -> AdmittedExecutable {
    AdmittedExecutable {
        binding: ResourceReferenceBinding {
            identity: request.executable.identity,
            version: request.executable.lifetime.version,
            content_profile: request.executable.content_profile.clone(),
            access_class: request.executable.access_class.clone(),
            handle: ResourceHandleId::from(format!("handle:{path}")),
            authority_contract: AuthorityContractId::from(JOB_EXECUTABLE_AUTHORITY),
            authority_grant: AuthorityGrantId::from("grant/job-test"),
            maximum_bytes: 1,
            maximum_items: Some(1),
            availability: ResourceReferenceAvailability::Available,
        },
        program: PathBuf::from(path),
    }
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
