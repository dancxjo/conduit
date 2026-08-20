//! Canonical Form catalog and exact hosted seam for bounded jobs.

use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, resource_requirement, ArtifactId, AuthorityContractId, AuthorityRequirement,
    CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId, HostOperationContractId,
    HostOperationRequirement, ImplementationId, ImplementationOffer, KindContractRevision,
    PortDescriptor, PortDirection, PortTemporal, StructuredInfoType,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_form::{KindDefinition, KindSignature};

use crate::{job_lifecycle_type, job_registered_types, job_request_type};

pub const JOB_FIXTURE_KIND: &str = "process/deterministic-request";
pub const JOB_RUN_KIND: &str = "process/run-bounded";
pub const JOB_REVISION: &str = "conduit.std/process-job@1";
pub const JOB_PROFILE: &str = "std/process-job-hosted@1";
pub const JOB_ARTIFACT: &str = "conduit-std-host/process-job@1";
pub const JOB_FIXTURE_OPERATION: &str = "conduit.host/process-job-fixture@1";
pub const JOB_RUN_OPERATION: &str = "conduit.host/process-job-run@1";
pub const JOB_EXECUTABLE_RESOURCE_CLASS: &str = "conduit.resource/executable@1";
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

pub fn job_std_offers() -> Vec<CapabilityOffer> {
    vec![
        offer(
            JOB_FIXTURE_KIND,
            JOB_FIXTURE_OPERATION,
            vec![],
            vec![port("request", &job_request_type(), PortDirection::Output)],
            false,
        ),
        offer(
            JOB_RUN_KIND,
            JOB_RUN_OPERATION,
            vec![port("request", &job_request_type(), PortDirection::Input)],
            vec![port(
                "lifecycle",
                &job_lifecycle_type(),
                PortDirection::Output,
            )],
            true,
        ),
    ]
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

fn offer(
    kind: &str,
    operation: &str,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
    executes: bool,
) -> CapabilityOffer {
    let operation = HostOperationRequirement {
        contract_id: HostOperationContractId::from(operation),
        target_kind: Some(kind_id(kind)),
        maximum_in_flight: 1,
        maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    };
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from(format!("std/{kind}@1")),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(JOB_REVISION),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(JOB_PROFILE),
            implementation_id: ImplementationId::from(format!("std/{kind}@1")),
            artifact_id: ArtifactId::from(JOB_ARTIFACT),
        },
        inputs,
        outputs,
        host_operations: vec![operation.clone()],
        resource_requirements: executes
            .then(|| resource_requirement(JOB_EXECUTABLE_RESOURCE_CLASS, 1))
            .into_iter()
            .collect(),
        authority_requirements: executes
            .then(|| AuthorityRequirement {
                contract_id: AuthorityContractId::from(JOB_EXECUTABLE_AUTHORITY),
                host_operation_contract_id: operation.contract_id,
                subject_kind: kind_id(JOB_RUN_KIND),
            })
            .into_iter()
            .collect(),
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: 4,
            max_queue_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 4) as u32,
        },
    }
}
