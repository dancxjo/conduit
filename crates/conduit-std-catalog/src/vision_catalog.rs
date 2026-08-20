//! Canonical Form catalog and finite std offers for image metadata.

use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, HostOperationContractId, HostOperationRequirement, ImplementationId,
    ImplementationOffer, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
    StructuredInfoType, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_form::{KindDefinition, KindSignature};

use crate::{
    image_resource_type, vision_detections_type, vision_registered_types, MAXIMUM_VISION_DETECTIONS,
};

pub const VISION_FIXTURE_KIND: &str = "vision/deterministic-image";
pub const VISION_DETECT_KIND: &str = "vision/deterministic-detector";
pub const VISION_REVISION: &str = "conduit.std/vision-metadata@1";
pub const VISION_PROFILE: &str = "std/vision-metadata-hosted@1";
pub const VISION_ARTIFACT: &str = "conduit-std-host/vision-metadata@1";
pub const VISION_HOST_OPERATION: &str = "conduit.host/vision-deterministic@1";

pub fn install_vision_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    for (name, value_type) in vision_registered_types() {
        startup
            .insert_structured_type(name, value_type)
            .map_err(|error| error.to_string())?;
    }
    insert_kind(
        startup,
        profile,
        VISION_FIXTURE_KIND,
        vec![],
        vec![port("image", &image_resource_type(), PortDirection::Output)],
    )?;
    insert_kind(
        startup,
        profile,
        VISION_DETECT_KIND,
        vec![port("image", &image_resource_type(), PortDirection::Input)],
        vec![port(
            "detections",
            &vision_detections_type(),
            PortDirection::Output,
        )],
    )
}

pub fn vision_std_offers() -> Vec<CapabilityOffer> {
    vec![
        offer(
            VISION_FIXTURE_KIND,
            vec![],
            vec![port("image", &image_resource_type(), PortDirection::Output)],
        ),
        offer(
            VISION_DETECT_KIND,
            vec![port("image", &image_resource_type(), PortDirection::Input)],
            vec![port(
                "detections",
                &vision_detections_type(),
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
            kind_contract_revision: KindContractRevision::from(VISION_REVISION),
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
            .expect("reviewed vision profile")
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
        kind_contract_revision: KindContractRevision::from(VISION_REVISION),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(VISION_PROFILE),
            implementation_id: ImplementationId::from(format!("std/{kind}@1")),
            artifact_id: ArtifactId::from(VISION_ARTIFACT),
        },
        inputs,
        outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(VISION_HOST_OPERATION),
            target_kind: Some(kind_id(kind)),
            maximum_in_flight: 1,
            maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: MAXIMUM_VISION_DETECTIONS,
            max_queue_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES
                * usize::from(MAXIMUM_VISION_DETECTIONS)) as u32,
        },
    }
}
