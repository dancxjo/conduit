//! Canonical Form catalog for finite schedule and workflow-state Info.

use alloc::{format, string::String, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, HostOperationContractId, HostOperationRequirement, ImplementationId,
    ImplementationOffer, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
    StructuredInfoType, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_form::{KindDefinition, KindSignature};

use crate::{
    schedule_assessment_type, schedule_observation_type, schedule_registered_types,
    scheduled_intent_type, workflow_lifecycle_type,
};

pub const SCHEDULE_FIXTURE_KIND: &str = "schedule/deterministic-fixture";
pub const SCHEDULE_ASSESS_KIND: &str = "schedule/assess-workflow";
pub const SCHEDULE_REVISION: &str = "conduit.std/schedule-workflow@1";
pub const SCHEDULE_PROFILE: &str = "std/schedule-workflow-hosted@1";
pub const SCHEDULE_ARTIFACT: &str = "conduit-std-host/schedule-workflow@1";
pub const SCHEDULE_HOST_OPERATION: &str = "conduit.host/schedule-workflow-pure@1";

pub fn install_schedule_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    for (name, value_type) in schedule_registered_types() {
        startup
            .insert_structured_type(name, value_type)
            .map_err(|error| error.to_string())?;
    }
    insert_kind(
        startup,
        profile,
        SCHEDULE_FIXTURE_KIND,
        vec![],
        vec![
            port("intent", &scheduled_intent_type(), PortDirection::Output),
            port(
                "lifecycle",
                &workflow_lifecycle_type(),
                PortDirection::Output,
            ),
            port(
                "observation",
                &schedule_observation_type(),
                PortDirection::Output,
            ),
        ],
    )?;
    insert_kind(
        startup,
        profile,
        SCHEDULE_ASSESS_KIND,
        vec![
            port("intent", &scheduled_intent_type(), PortDirection::Input),
            port(
                "lifecycle",
                &workflow_lifecycle_type(),
                PortDirection::Input,
            ),
            port(
                "observation",
                &schedule_observation_type(),
                PortDirection::Input,
            ),
        ],
        vec![port(
            "assessment",
            &schedule_assessment_type(),
            PortDirection::Output,
        )],
    )
}

pub fn schedule_std_offers() -> Vec<CapabilityOffer> {
    vec![
        offer(
            SCHEDULE_FIXTURE_KIND,
            vec![],
            vec![
                port("intent", &scheduled_intent_type(), PortDirection::Output),
                port(
                    "lifecycle",
                    &workflow_lifecycle_type(),
                    PortDirection::Output,
                ),
                port(
                    "observation",
                    &schedule_observation_type(),
                    PortDirection::Output,
                ),
            ],
        ),
        offer(
            SCHEDULE_ASSESS_KIND,
            vec![
                port("intent", &scheduled_intent_type(), PortDirection::Input),
                port(
                    "lifecycle",
                    &workflow_lifecycle_type(),
                    PortDirection::Input,
                ),
                port(
                    "observation",
                    &schedule_observation_type(),
                    PortDirection::Input,
                ),
            ],
            vec![port(
                "assessment",
                &schedule_assessment_type(),
                PortDirection::Output,
            )],
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
            kind_contract_revision: KindContractRevision::from(SCHEDULE_REVISION),
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
            .expect("reviewed schedule profile")
            .value_kind()
            .clone(),
        direction,
        temporal: PortTemporal::Value,
    }
}

fn offer(kind: &str, inputs: Vec<PortDescriptor>, outputs: Vec<PortDescriptor>) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from(format!("std/{kind}@1")),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(SCHEDULE_REVISION),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(SCHEDULE_PROFILE),
            implementation_id: ImplementationId::from(format!("std/{kind}@1")),
            artifact_id: ArtifactId::from(SCHEDULE_ARTIFACT),
        },
        inputs,
        outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(SCHEDULE_HOST_OPERATION),
            target_kind: Some(kind_id(kind)),
            maximum_in_flight: 1,
            maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: 4,
            max_queue_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 4) as u32,
        },
    }
}
