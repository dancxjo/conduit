//! Form catalog, deterministic offers, and the explicit physical-motion seam.

use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, ArtifactId, AuthorityContractId, AuthorityRequirement, CapabilityId,
    CapabilityLimits, CapabilityOffer, ExecutionProfileId, HostOperationContractId,
    HostOperationRequirement, ImplementationId, ImplementationOffer, KindContractRevision,
    PortDescriptor, PortDirection, PortTemporal, StructuredInfoType,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_form::{KindDefinition, KindSignature};

use crate::{
    robotics_contact_event_type, robotics_motion_request_type, robotics_pose_sample_type,
    robotics_power_telemetry_type, robotics_range_observation_type,
    robotics_structured_registered_types,
};

pub const DETERMINISTIC_ROBOTICS_OBSERVATIONS_KIND: &str =
    "robotics/deterministic-observations";
pub const DETERMINISTIC_ROBOTICS_INTENT_KIND: &str = "robotics/deterministic-intent";
pub const ROBOTICS_EXECUTE_MOTION_KIND: &str = "robotics/execute-motion";
pub const ROBOTICS_STRUCTURED_REVISION: &str = "conduit.std/robotics-structured@1";
pub const ROBOTICS_STRUCTURED_PROFILE: &str = "std/robotics-structured-deterministic@1";
pub const ROBOTICS_STRUCTURED_ARTIFACT: &str = "conduit-std-host/robotics-structured@1";
pub const ROBOTICS_STRUCTURED_HOST_OPERATION: &str =
    "conduit.host/robotics-structured-deterministic@1";
pub const ROBOTICS_PHYSICAL_MOTION_PROFILE: &str = "robotics-provider/physical-motion@1";
pub const ROBOTICS_PHYSICAL_MOTION_ARTIFACT: &str = "robotics-provider/physical-motion@1";
pub const ROBOTICS_PHYSICAL_MOTION_HOST_OPERATION: &str = "conduit.host/robotics-motion@1";
pub const ROBOTICS_PHYSICAL_MOTION_AUTHORITY: &str = "robotics/actuate-motion@1";

pub fn install_robotics_structured_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    for (name, value_type) in robotics_structured_registered_types() {
        startup
            .insert_structured_type(name, value_type)
            .map_err(|error| error.to_string())?;
    }
    insert_kind(
        startup,
        profile,
        DETERMINISTIC_ROBOTICS_OBSERVATIONS_KIND,
        vec![],
        observation_outputs(),
    )?;
    insert_kind(
        startup,
        profile,
        DETERMINISTIC_ROBOTICS_INTENT_KIND,
        vec![],
        vec![port(
            "request",
            &robotics_motion_request_type(),
            PortDirection::Output,
        )],
    )?;
    insert_kind(
        startup,
        profile,
        ROBOTICS_EXECUTE_MOTION_KIND,
        vec![port(
            "request",
            &robotics_motion_request_type(),
            PortDirection::Input,
        )],
        vec![],
    )
}

pub fn robotics_structured_deterministic_offers() -> Vec<CapabilityOffer> {
    vec![
        offer(
            DETERMINISTIC_ROBOTICS_OBSERVATIONS_KIND,
            vec![],
            observation_outputs(),
            ROBOTICS_STRUCTURED_PROFILE,
            ROBOTICS_STRUCTURED_ARTIFACT,
            ROBOTICS_STRUCTURED_HOST_OPERATION,
            false,
        ),
        offer(
            DETERMINISTIC_ROBOTICS_INTENT_KIND,
            vec![],
            vec![port(
                "request",
                &robotics_motion_request_type(),
                PortDirection::Output,
            )],
            ROBOTICS_STRUCTURED_PROFILE,
            ROBOTICS_STRUCTURED_ARTIFACT,
            ROBOTICS_STRUCTURED_HOST_OPERATION,
            false,
        ),
    ]
}

/// Provider-facing offer seam. It is deliberately not installed by std: a
/// physical provider must supply the implementation and an exact authority
/// grant before this semantic actuator request can be planned.
pub fn robotics_physical_motion_offer() -> CapabilityOffer {
    offer(
        ROBOTICS_EXECUTE_MOTION_KIND,
        vec![port(
            "request",
            &robotics_motion_request_type(),
            PortDirection::Input,
        )],
        vec![],
        ROBOTICS_PHYSICAL_MOTION_PROFILE,
        ROBOTICS_PHYSICAL_MOTION_ARTIFACT,
        ROBOTICS_PHYSICAL_MOTION_HOST_OPERATION,
        true,
    )
}

fn observation_outputs() -> Vec<PortDescriptor> {
    vec![
        port(
            "contact",
            &robotics_contact_event_type(),
            PortDirection::Output,
        ),
        port(
            "pose",
            &robotics_pose_sample_type(),
            PortDirection::Output,
        ),
        port(
            "power",
            &robotics_power_telemetry_type(),
            PortDirection::Output,
        ),
        port(
            "range",
            &robotics_range_observation_type(),
            PortDirection::Output,
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
            kind_contract_revision: KindContractRevision::from(ROBOTICS_STRUCTURED_REVISION),
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
            .expect("reviewed robotics structured profile")
            .value_kind()
            .clone(),
        direction,
        temporal: PortTemporal::Value,
    }
}

#[allow(clippy::too_many_arguments)]
fn offer(
    kind: &str,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
    profile: &str,
    artifact: &str,
    host_operation: &str,
    physical_motion: bool,
) -> CapabilityOffer {
    let operation = HostOperationRequirement {
        contract_id: HostOperationContractId::from(host_operation),
        target_kind: Some(kind_id(kind)),
        maximum_in_flight: 1,
        maximum_input_bytes: if inputs.is_empty() {
            0
        } else {
            MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32
        },
        maximum_output_bytes: if outputs.is_empty() {
            0
        } else {
            MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32
        },
    };
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from(format!("{profile}/{kind}")),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(ROBOTICS_STRUCTURED_REVISION),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(profile),
            implementation_id: ImplementationId::from(format!("{profile}/{kind}")),
            artifact_id: ArtifactId::from(artifact),
        },
        inputs,
        outputs,
        host_operations: vec![operation.clone()],
        resource_requirements: vec![],
        authority_requirements: physical_motion
            .then(|| AuthorityRequirement {
                contract_id: AuthorityContractId::from(ROBOTICS_PHYSICAL_MOTION_AUTHORITY),
                host_operation_contract_id: operation.contract_id,
                subject_kind: kind_id(ROBOTICS_EXECUTE_MOTION_KIND),
            })
            .into_iter()
            .collect(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        },
    }
}
