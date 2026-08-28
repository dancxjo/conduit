//! Canonical portable Form catalog for image metadata.

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, KindContractRevision, KindId, PortDescriptor, PortDirection, PortTemporal,
    StructuredInfoType,
};
use conduit_form::{KindDefinition, KindSignature};

use crate::{image_resource_type, vision_detections_type, vision_registered_types};

pub const VISION_FIXTURE_KIND: &str = "vision/deterministic-image";
pub const VISION_DETECT_KIND: &str = "vision/deterministic-detector";
pub const VISION_REVISION: &str = "conduit.std/vision-metadata@1";

pub type VisionKindContract = (KindId, Vec<PortDescriptor>, Vec<PortDescriptor>);

/// Exact portable vision Kinds and typed faces, without any Host realization facts.
pub fn vision_kind_contracts() -> Vec<VisionKindContract> {
    vec![
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
    ]
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
