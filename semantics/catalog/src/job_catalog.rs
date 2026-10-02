//! Canonical Plot catalog and exact hosted seam for bounded jobs.

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, Kind, KindIdentity, PortDescriptor, PortDirection,
    PortTemporal, StructuredInfoType, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_plot::{KindProjection, KindSignature};

use crate::{JobLifecycleEvent, JobOutput, JobOutputProfile, JobRequest, JobResourceUsage};

pub const JOB_REQUEST_TYPE: &str = "JobRequest";
pub const JOB_LIFECYCLE_TYPE: &str = "JobLifecycle";
pub const JOB_OUTPUT_TYPE: &str = "JobOutput";
pub const JOB_USAGE_TYPE: &str = "JobResourceUsage";

pub const JOB_FIXTURE_KIND: &str = "process/deterministic-request";
pub const JOB_RUN_KIND: &str = "process/run-bounded";
pub const JOB_REVISION: &str = "conduit.std/process-job@1";

pub fn job_output_profile_type() -> StructuredInfoType {
    JobOutputProfile::semantic_type().expect("checked Process output profile")
}

pub fn job_request_type() -> StructuredInfoType {
    JobRequest::semantic_type().expect("checked Process Job request")
}

pub fn job_output_type() -> StructuredInfoType {
    JobOutput::semantic_type().expect("checked Process Job output")
}

pub fn job_usage_type() -> StructuredInfoType {
    JobResourceUsage::semantic_type().expect("checked Process resource usage")
}

pub fn job_lifecycle_type() -> StructuredInfoType {
    JobLifecycleEvent::semantic_type().expect("checked Process Job lifecycle")
}

pub fn job_registered_types() -> Vec<(&'static str, StructuredInfoType)> {
    vec![
        (JOB_REQUEST_TYPE, job_request_type()),
        (JOB_LIFECYCLE_TYPE, job_lifecycle_type()),
        (JOB_OUTPUT_TYPE, job_output_type()),
        (JOB_USAGE_TYPE, job_usage_type()),
    ]
}
pub fn job_semantic_contracts() -> Vec<Kind> {
    vec![
        contract(
            JOB_FIXTURE_KIND,
            vec![],
            vec![port("request", &job_request_type(), PortDirection::Output)],
        ),
        contract(
            JOB_RUN_KIND,
            vec![port("request", &job_request_type(), PortDirection::Input)],
            vec![port(
                "lifecycle",
                &job_lifecycle_type(),
                PortDirection::Output,
            )],
        ),
    ]
}

pub fn install_job_catalogs(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    for (name, value_type) in job_registered_types() {
        startup
            .insert_structured_type(name, value_type)
            .map_err(|error| error.to_string())?;
    }
    for contract in job_semantic_contracts() {
        insert_kind(startup, profile, contract)?;
    }
    Ok(())
}

fn insert_kind(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
    contract: Kind,
) -> Result<(), String> {
    startup
        .insert(KindSignature {
            kind: contract.kind_id.as_str().into(),
            startup_parameters: vec![],
        })
        .map_err(|error| error.to_string())?;
    profile
        .insert(KindProjection {
            kind_id: contract.kind_id,
            kind_contract_revision: contract.kind_contract_revision,
            inputs: contract.inputs,
            outputs: contract.outputs,
            configuration: vec![],
        })
        .map_err(|error| error.to_string())
}

fn contract(kind: &str, inputs: Vec<PortDescriptor>, outputs: Vec<PortDescriptor>) -> Kind {
    Kind {
        startup_parameters: vec![],
        shorthand: None,
        kind_id: kind_id(kind),
        kind_contract_revision: KindIdentity::from(JOB_REVISION),
        inputs,
        outputs,
        configuration: Default::default(),
        semantic_laws: Default::default(),
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: 4,
            max_queue_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 4) as u32,
        },
    }
}

fn port(name: &str, value_type: &StructuredInfoType, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: value_type
            .profile()
            .expect("reviewed job profile")
            .value_kind()
            .clone(),
        direction,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    }
}
