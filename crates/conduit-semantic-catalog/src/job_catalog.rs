//! Canonical Form catalog and exact hosted seam for bounded jobs.

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
    StructuredInfoType,
};
use conduit_form::{KindDefinition, KindSignature};

use crate::{job_lifecycle_type, job_registered_types, job_request_type};

pub const JOB_FIXTURE_KIND: &str = "process/deterministic-request";
pub const JOB_RUN_KIND: &str = "process/run-bounded";
pub const JOB_REVISION: &str = "conduit.std/process-job@1";
pub const JOB_EXECUTABLE_AUTHORITY: &str = "conduit.authority/execute-resource@1";

pub fn install_job_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    for (name, value_type) in job_registered_types() {
        startup
            .insert_structured_type(name, value_type)
            .map_err(|error| error.to_string())?;
    }
    insert_kind(
        startup,
        profile,
        JOB_FIXTURE_KIND,
        vec![],
        vec![port("request", &job_request_type(), PortDirection::Output)],
    )?;
    insert_kind(
        startup,
        profile,
        JOB_RUN_KIND,
        vec![port("request", &job_request_type(), PortDirection::Input)],
        vec![port(
            "lifecycle",
            &job_lifecycle_type(),
            PortDirection::Output,
        )],
    )
}

fn insert_kind(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
    kind: &str,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
) -> Result<(), String> {
    startup
        .insert(KindSignature {
            kind: kind.into(),
            startup_parameters: vec![],
        })
        .map_err(|error| error.to_string())?;
    profile
        .insert(KindDefinition {
            kind_id: kind_id(kind),
            kind_contract_revision: KindContractRevision::from(JOB_REVISION),
            inputs,
            outputs,
            configuration: vec![],
        })
        .map_err(|error| error.to_string())
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
    }
}
