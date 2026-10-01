use conduit_core::{
    kind_id, BoundedResourceRef, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity,
};
use conduit_form::rust_binding::{BoundedBytes, BoundedSequence, NativeRustBinding};
use conduit_process::{
    JobArguments, JobEnvironment, JobEnvironmentEntry, JobExecutable, JobOutputBytes, JobRequest,
    JobRequestRefusal, JobText, JOB_EXECUTABLE_ACCESS_CLASS, JOB_EXECUTABLE_CONTENT_PROFILE,
    JOB_MAXIMUM_OUTPUT_BYTES,
};

#[test]
fn authored_job_request_round_trips_through_generated_binding() {
    let request = request(
        [text("--exact")],
        [JobEnvironmentEntry::new(text("MODE"), text("bounded")).unwrap()],
    );
    request.validate_job().unwrap();

    let structured = request.clone().into_structured().unwrap();
    assert_eq!(JobRequest::from_structured(structured).unwrap(), request);
}

#[test]
fn exact_argument_and_environment_bounds_are_native() {
    let arguments = core::array::from_fn::<_, 8, _>(|index| text(&format!("arg-{index}")));
    let environment = core::array::from_fn::<_, 8, _>(|index| {
        JobEnvironmentEntry::new(text(&format!("KEY_{index}")), text("value")).unwrap()
    });
    let request = request(arguments, environment);

    request.validate_job().unwrap();
    assert_eq!(request.arguments().get().len(), 8);
    assert_eq!(request.environment().get().len(), 8);
}

#[test]
fn contextual_executable_and_environment_rules_remain_explicit() {
    let duplicate = JobEnvironmentEntry::new(text("MODE"), text("one")).unwrap();
    let request = request([], [duplicate.clone(), duplicate]);
    assert_eq!(
        request.validate_job(),
        Err(JobRequestRefusal::DuplicateEnvironmentName)
    );

    let mut executable = executable();
    executable.access_class = ResourceClassId::from("conduit.resource/not-executable@1");
    let request = request_with_executable([], [], executable);
    assert_eq!(
        request.validate_job(),
        Err(JobRequestRefusal::InvalidExecutable)
    );
}

#[test]
fn exact_maximum_output_payload_survives_the_generated_binding() {
    let bytes = vec![0x5a; JOB_MAXIMUM_OUTPUT_BYTES as usize];
    let bounded = BoundedBytes::<65536>::new(&bytes).unwrap();
    let output = JobOutputBytes::new(bounded).unwrap();
    let structured = output.clone().into_structured().unwrap();

    assert_eq!(JobOutputBytes::from_structured(structured).unwrap(), output);
}

#[test]
fn generated_constructors_refuse_every_over_bound_value() {
    assert!(JobText::new("x".repeat(257)).is_err());
    assert!(BoundedBytes::<65536>::new(&vec![0; 65_537]).is_none());
    assert!(BoundedSequence::<JobText, 8>::try_from_iter(
        (0..9).map(|index| text(&format!("arg-{index}")))
    )
    .is_err());
}

fn request<const A: usize, const E: usize>(
    arguments: [JobText; A],
    environment: [JobEnvironmentEntry; E],
) -> JobRequest {
    request_with_executable(arguments, environment, executable())
}

fn request_with_executable<const A: usize, const E: usize>(
    arguments: [JobText; A],
    environment: [JobEnvironmentEntry; E],
    executable: BoundedResourceRef,
) -> JobRequest {
    JobRequest::new(
        JobArguments::new(BoundedSequence::try_from_iter(arguments).unwrap()).unwrap(),
        JobEnvironment::new(BoundedSequence::try_from_iter(environment).unwrap()).unwrap(),
        JobExecutable::new(executable).unwrap(),
        1_024,
        JOB_MAXIMUM_OUTPUT_BYTES,
        conduit_process::JobOutputProfile::Utf8,
        conduit_process::JobOutputProfile::Bytes,
        1_000,
    )
    .unwrap()
}

fn executable() -> BoundedResourceRef {
    let digest = [7; 32];
    BoundedResourceRef {
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
    }
}

fn text(value: &str) -> JobText {
    JobText::new(value.into()).unwrap()
}
