//! Portable bounded job semantics.
//!
//! An executable is semantic resource Info, not a path or command line. A Host
//! must separately admit and bind that reference before realizing the request.

use alloc::{string::String, vec, vec::Vec};
use conduit_core::{
    kind_id, BoundedResourceRef, StructuredFieldType, StructuredInfoType,
    StructuredVariantCase, RESOURCE_REFERENCE_INFO_ID,
};

pub const JOB_REQUEST_TYPE: &str = "JobRequest";
pub const JOB_LIFECYCLE_TYPE: &str = "JobLifecycle";
pub const JOB_OUTPUT_TYPE: &str = "JobOutput";
pub const JOB_USAGE_TYPE: &str = "JobResourceUsage";
pub const JOB_ARGUMENT_SLOTS: usize = 8;
pub const JOB_ENVIRONMENT_SLOTS: usize = 8;
pub const JOB_MAXIMUM_TEXT_BYTES: usize = 256;
pub const JOB_MAXIMUM_OUTPUT_BYTES: u32 = 65_536;
pub const JOB_MAXIMUM_TIMEOUT_MILLIS: u64 = 86_400_000;
pub const JOB_EXECUTABLE_CONTENT_PROFILE: &str = "process/executable-image@1";
pub const JOB_EXECUTABLE_ACCESS_CLASS: &str = "conduit.resource/executable@1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobOutputProfile {
    Bytes,
    Utf8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobEnvironmentEntry {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobRequest {
    pub executable: BoundedResourceRef,
    pub arguments: Vec<String>,
    pub environment: Vec<JobEnvironmentEntry>,
    pub stdout_profile: JobOutputProfile,
    pub stderr_profile: JobOutputProfile,
    pub maximum_stdout_bytes: u32,
    pub maximum_stderr_bytes: u32,
    pub timeout_millis: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobRequestRefusal {
    InvalidExecutable,
    TooManyArguments,
    TooManyEnvironmentEntries,
    EmptyEnvironmentName,
    DuplicateEnvironmentName,
    TextTooLarge,
    OutputBoundExceeded,
    InvalidTimeout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobStreamPressure {
    WithinLimit,
    Truncated { observed_minimum_bytes: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobOutput {
    pub profile: JobOutputProfile,
    pub bytes: Vec<u8>,
    pub pressure: JobStreamPressure,
    pub complete_artifact: Option<BoundedResourceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobResourceUsage {
    pub elapsed_millis: u64,
    pub stdout_observed_bytes: u64,
    pub stderr_observed_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobExitDisposition {
    ExitCode(i32),
    Signal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobTerminalOutcome {
    Completed { disposition: JobExitDisposition },
    Failed { disposition: JobExitDisposition, message: String },
    Cancelled { message: String },
    TimedOut { timeout_millis: u64, message: String },
    ProviderLost { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobLifecycleEvent {
    Started,
    Running,
    Terminal(JobTerminalOutcome),
}

impl JobRequest {
    pub fn validate(&self) -> Result<(), JobRequestRefusal> {
        self.executable
            .validate()
            .map_err(|_| JobRequestRefusal::InvalidExecutable)?;
        if self.executable.content_profile.as_str() != JOB_EXECUTABLE_CONTENT_PROFILE
            || self.executable.access_class.as_str() != JOB_EXECUTABLE_ACCESS_CLASS
        {
            return Err(JobRequestRefusal::InvalidExecutable);
        }
        if self.arguments.len() > JOB_ARGUMENT_SLOTS {
            return Err(JobRequestRefusal::TooManyArguments);
        }
        if self.environment.len() > JOB_ENVIRONMENT_SLOTS {
            return Err(JobRequestRefusal::TooManyEnvironmentEntries);
        }
        for (index, entry) in self.environment.iter().enumerate() {
            if entry.name.is_empty() {
                return Err(JobRequestRefusal::EmptyEnvironmentName);
            }
            if self.environment[..index]
                .iter()
                .any(|previous| previous.name == entry.name)
            {
                return Err(JobRequestRefusal::DuplicateEnvironmentName);
            }
            validate_text(&entry.name)?;
            validate_text(&entry.value)?;
        }
        for argument in &self.arguments {
            validate_text(argument)?;
        }
        if self.maximum_stdout_bytes > JOB_MAXIMUM_OUTPUT_BYTES
            || self.maximum_stderr_bytes > JOB_MAXIMUM_OUTPUT_BYTES
        {
            return Err(JobRequestRefusal::OutputBoundExceeded);
        }
        if self.timeout_millis == 0 || self.timeout_millis > JOB_MAXIMUM_TIMEOUT_MILLIS {
            return Err(JobRequestRefusal::InvalidTimeout);
        }
        Ok(())
    }
}

fn validate_text(value: &str) -> Result<(), JobRequestRefusal> {
    if value.len() > JOB_MAXIMUM_TEXT_BYTES {
        Err(JobRequestRefusal::TextTooLarge)
    } else {
        Ok(())
    }
}

fn leaf(kind: &str) -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(kind)).expect("reviewed job leaf")
}

fn field(name: &str, value_type: StructuredInfoType) -> StructuredFieldType {
    StructuredFieldType::new(name, value_type).expect("reviewed job field")
}

fn case(name: &str, payload_type: StructuredInfoType) -> StructuredVariantCase {
    StructuredVariantCase::new(name, payload_type).expect("reviewed job case")
}

fn record(kind: &str, fields: Vec<StructuredFieldType>) -> StructuredInfoType {
    StructuredInfoType::record(kind_id(kind), fields).expect("reviewed job record")
}

fn unit_type() -> StructuredInfoType {
    leaf("value/unit@1")
}

fn text_type() -> StructuredInfoType {
    leaf("value/text@1")
}

fn count_type() -> StructuredInfoType {
    leaf("value/count@1")
}

fn optional_text_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("process/optional-text@1"),
        vec![case("absent", unit_type()), case("present", text_type())],
    )
    .expect("reviewed optional job text")
}

fn optional_environment_type() -> StructuredInfoType {
    let entry = record(
        "process/environment-entry@1",
        vec![field("name", text_type()), field("value", text_type())],
    );
    StructuredInfoType::variant(
        kind_id("process/optional-environment-entry@1"),
        vec![case("absent", unit_type()), case("present", entry)],
    )
    .expect("reviewed optional environment entry")
}

pub fn job_output_profile_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("process/output-profile@1"),
        vec![case("bytes", unit_type()), case("utf8", unit_type())],
    )
    .expect("reviewed output profile")
}

pub fn job_request_type() -> StructuredInfoType {
    record(
        "process/job-request@1",
        vec![
            field(
                "arguments",
                StructuredInfoType::collection(optional_text_type(), Some(JOB_ARGUMENT_SLOTS as u16))
                    .expect("bounded argument slots"),
            ),
            field(
                "environment",
                StructuredInfoType::collection(
                    optional_environment_type(),
                    Some(JOB_ENVIRONMENT_SLOTS as u16),
                )
                .expect("bounded environment slots"),
            ),
            field("executable", leaf(RESOURCE_REFERENCE_INFO_ID)),
            field("maximum_stderr_bytes", count_type()),
            field("maximum_stdout_bytes", count_type()),
            field("stderr_profile", job_output_profile_type()),
            field("stdout_profile", job_output_profile_type()),
            field("timeout_millis", count_type()),
        ],
    )
}

pub fn job_output_type() -> StructuredInfoType {
    let artifact = StructuredInfoType::variant(
        kind_id("process/optional-output-artifact@1"),
        vec![case("absent", unit_type()), case("resource", leaf(RESOURCE_REFERENCE_INFO_ID))],
    )
    .expect("reviewed output artifact");
    let pressure = StructuredInfoType::variant(
        kind_id("process/stream-pressure@1"),
        vec![case("truncated", count_type()), case("within_limit", unit_type())],
    )
    .expect("reviewed stream pressure");
    record(
        "process/job-output@1",
        vec![
            field("complete_artifact", artifact),
            field("inline", leaf("value/bytes@1")),
            field("pressure", pressure),
            field("profile", job_output_profile_type()),
        ],
    )
}

pub fn job_usage_type() -> StructuredInfoType {
    record(
        "process/job-resource-usage@1",
        vec![
            field("elapsed_millis", count_type()),
            field("stderr_observed_bytes", count_type()),
            field("stdout_observed_bytes", count_type()),
        ],
    )
}

fn exit_disposition_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("process/exit-disposition@1"),
        vec![case("exit_code", leaf("value/integer@1")), case("signal", unit_type())],
    )
    .expect("reviewed exit disposition")
}

fn terminal_detail_type(kind: &str, timed: bool) -> StructuredInfoType {
    let mut fields = vec![field("message", text_type()), field("usage", job_usage_type())];
    if timed {
        fields.push(field("timeout_millis", count_type()));
    }
    record(kind, fields)
}

pub fn job_lifecycle_type() -> StructuredInfoType {
    let completed = record(
        "process/job-completed@1",
        vec![
            field("disposition", exit_disposition_type()),
            field("stderr", job_output_type()),
            field("stdout", job_output_type()),
            field("usage", job_usage_type()),
        ],
    );
    let failed = record(
        "process/job-failed@1",
        vec![
            field("disposition", exit_disposition_type()),
            field("message", text_type()),
            field("stderr", job_output_type()),
            field("stdout", job_output_type()),
            field("usage", job_usage_type()),
        ],
    );
    StructuredInfoType::variant(
        kind_id("process/job-lifecycle@1"),
        vec![
            case("cancelled", terminal_detail_type("process/job-cancelled@1", false)),
            case("completed", completed),
            case("failed", failed),
            case("provider_lost", terminal_detail_type("process/job-provider-lost@1", false)),
            case("running", unit_type()),
            case("started", unit_type()),
            case("timed_out", terminal_detail_type("process/job-timed-out@1", true)),
        ],
    )
    .expect("reviewed job lifecycle")
}

pub fn job_registered_types() -> Vec<(&'static str, StructuredInfoType)> {
    vec![
        (JOB_REQUEST_TYPE, job_request_type()),
        (JOB_LIFECYCLE_TYPE, job_lifecycle_type()),
        (JOB_OUTPUT_TYPE, job_output_type()),
        (JOB_USAGE_TYPE, job_usage_type()),
    ]
}
