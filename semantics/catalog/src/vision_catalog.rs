//! Canonical portable form catalog for image metadata.

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, Kind, KindId, KindIdentity, PortDescriptor, PortDirection,
    PortTemporal, StructuredInfoType, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_form::{KindDefinition, KindSignature};

use crate::{
    flow_coalesce_latest_contract, image_resource_type, install_flow_pressure_kind,
    vision_detections_type, vision_experience_kind_contracts, vision_experience_registered_types,
    vision_registered_types,
};

pub const VISION_FIXTURE_KIND: &str = "vision/deterministic-image";
pub const VISION_DETECT_KIND: &str = "vision/deterministic-detector";
pub const VISION_REVISION: &str = "conduit.std/vision-metadata@1";
pub const VISION_LOCAL_OBJECTS_REVISION: &str = "conduit.semantic/vision-local-objects@2";

pub fn vision_kind_revision(kind: &str) -> &'static str {
    if kind == crate::VISION_OBJECTS_KIND {
        VISION_LOCAL_OBJECTS_REVISION
    } else {
        VISION_REVISION
    }
}

pub type VisionKindContract = (KindId, Vec<PortDescriptor>, Vec<PortDescriptor>);

/// Exact portable vision Kinds and typed fronts, without any Host realization facts.
pub fn vision_kind_contracts() -> Vec<VisionKindContract> {
    let mut contracts = vec![
        (
            kind_id(VISION_FIXTURE_KIND),
            vec![],
            vec![port("image", &image_resource_type(), PortDirection::Output)],
        ),
        (
            kind_id(VISION_DETECT_KIND),
            vec![port("image", &image_resource_type(), PortDirection::Input)],
            vec![port(
                "detections",
                &vision_detections_type(),
                PortDirection::Output,
            )],
        ),
    ];
    contracts.extend(vision_experience_kind_contracts());
    contracts
}

pub fn vision_semantic_contracts() -> Vec<Kind> {
    vision_kind_contracts()
        .into_iter()
        .map(|(kind_id, inputs, outputs)| {
            let revision = vision_kind_revision(kind_id.as_str());
            Kind {
                startup_parameters: vec![],
                shorthand: None,
                kind_id,
                kind_contract_revision: KindIdentity::from(revision),
                inputs,
                outputs,
                limits: CapabilityLimits {
                    max_active_instances: 1,
                    max_queue_items: 1,
                    max_queue_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
                },
            }
        })
        .collect()
}

pub fn install_vision_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    for (name, value_type) in vision_registered_types() {
        startup
            .insert_structured_type(name, value_type)
            .map_err(|error| error.to_string())?;
    }
    for (name, value_type) in vision_experience_registered_types() {
        startup
            .insert_structured_type(name, value_type)
            .map_err(|error| error.to_string())?;
    }
    install_flow_pressure_kind(
        flow_coalesce_latest_contract(
            image_resource_type()
                .profile()
                .expect("reviewed image profile")
                .value_kind(),
            conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        ),
        startup,
        profile,
    )?;
    for (kind, inputs, outputs) in vision_kind_contracts() {
        insert_kind(startup, profile, kind.as_str(), inputs, outputs)?;
    }
    Ok(())
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
            kind_contract_revision: KindIdentity::from(vision_kind_revision(kind)),
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
