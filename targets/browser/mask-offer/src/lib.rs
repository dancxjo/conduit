//! Reviewed Back and ordinary Plot Plan for the browser's bounded DOM Mask.

#[cfg(test)]
use conduit_core::HostAdvertisement;
#[cfg(test)]
use conduit_plot::ProfileCatalog;
#[cfg(test)]
use std::collections::BTreeMap;

use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityLimits, ExecutionProfileId,
    FACE_RESOURCE_CLASS, HostCallContractId, HostCallRequirement, ImplementationId, Kind,
    KindIdentity, PortDescriptor, PortDirection, PortTemporal, kind_id, port_id,
    resource_requirement,
};
use conduit_face::{FACE_INTERACTION_VALUE_KIND, FACE_VALUE_KIND, SHOW_VALUE_KIND};

mod configured_sources;
mod planning;
#[cfg(test)]
use planning::planned_mask_configuration;
use planning::{OwnerCarrierForeLimits, planned_mask_configuration_with_look};
pub use planning::{
    configured_mask_configuration, configured_mask_configuration_with_look, planned_mask,
};
mod remote;
pub use remote::{
    planned_owner_face_show_interaction_mask, planned_owner_face_show_interaction_mask_with_look,
    planned_owner_face_show_mask,
};

pub const MASK_BYTES: u32 = 512 * 1024;
const MASK_OPERATION: &str = "browser.host/dom-mask@1";

fn port(
    name: &str,
    value_kind: &str,
    direction: PortDirection,
    temporal: PortTemporal,
) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal,
        abnormal_kind: None,
    }
}

pub fn kind() -> Kind {
    Kind {
        startup_parameters: vec![],
        shorthand: None,
        kind_id: kind_id("face/browser-dom-mask"),
        kind_contract_revision: KindIdentity::from("conduit.browser/face-dom-mask@1"),
        inputs: vec![port(
            "face",
            FACE_VALUE_KIND,
            PortDirection::Input,
            PortTemporal::Value,
        )],
        outputs: vec![
            port(
                "interaction",
                FACE_INTERACTION_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Flow { closes: true },
            ),
            port(
                "show",
                SHOW_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Value,
            ),
        ],
        configuration: vec![],
        semantic_laws: Default::default(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 4,
            max_queue_bytes: MASK_BYTES,
        },
    }
}

pub fn offer() -> conduit_core::CapabilityOffer {
    BackOfferBuilder::new(
        kind(),
        Back {
            capability_id: CapabilityId::from("capability/browser-dom-mask"),
            execution_profile_id: ExecutionProfileId::from("browser/mask@1"),
            implementation_id: ImplementationId::from("implementation/browser-dom-mask"),
            artifact_id: ArtifactId::from("artifact/browser-runtime"),
            host_calls: vec![HostCallRequirement {
                contract_id: HostCallContractId::from(MASK_OPERATION),
                target_kind: Some(kind_id("face/browser-dom-mask")),
                maximum_in_flight: 1,
                maximum_input_bytes: MASK_BYTES,
                maximum_output_bytes: MASK_BYTES,
            }],
            resource_requirements: vec![resource_requirement(FACE_RESOURCE_CLASS, 1)],
            authority_requirements: vec![],
        },
    )
    .build()
}

pub const MASK_SOURCE: &str = "plot browser-graphical (\n >> face: Face\n interaction: FaceInteraction...| >>\n show: Show >>\n) {\n mask: face/browser-dom-mask\n face >> mask.face\n mask.interaction >> interaction\n mask.show >> show\n}\n";
pub const ALTERNATE_MASK_SOURCE: &str = "plot browser-graphical-alternate (\n >> face: Face\n interaction: FaceInteraction...| >>\n show: Show >>\n) {\n mask: face/browser-dom-mask\n face >> mask.face\n mask.interaction >> interaction\n mask.show >> show\n}\n";

/// Actual browser Back of the portable, source-configured component Fore.
pub fn configured_offer() -> conduit_core::CapabilityOffer {
    let original = offer();
    let mut back = Back {
        capability_id: original.capability_id,
        execution_profile_id: original.implementation.execution_profile_id,
        implementation_id: original.implementation.implementation_id,
        artifact_id: original.implementation.artifact_id,
        host_calls: original.host_calls,
        resource_requirements: original.resource_requirements,
        authority_requirements: original.authority_requirements,
    };
    back.capability_id = CapabilityId::from("capability/browser-component-mask");
    back.implementation_id = ImplementationId::from("implementation/browser-component-mask");
    for requirement in &mut back.host_calls {
        requirement.target_kind = Some(kind_id(conduit_face::COMPONENT_MASK_KIND));
    }
    BackOfferBuilder::new(conduit_face::component_mask_kind(MASK_BYTES), back).build()
}

#[cfg(test)]
mod configured_tests;

#[cfg(test)]
mod topology_tests;
