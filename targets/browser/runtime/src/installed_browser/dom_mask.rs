//! Installed Back for the browser's bounded DOM Mask executor.

use conduit_core::{
    kind_id, port_id, resource_requirement, ArtifactId, Back, BackOfferBuilder, CapabilityId,
    CapabilityLimits, ExecutionProfileId, HostCallContractId, HostCallRequirement,
    ImplementationId, Kind, KindIdentity, PortDescriptor, PortDirection, PortTemporal,
    PRESENTATION_RESOURCE_CLASS,
};
use conduit_presentation::{FACE_INTERACTION_VALUE_KIND, PRESENTATION_VALUE_KIND, SHOW_VALUE_KIND};

pub(crate) const MASK_BYTES: u32 = 512 * 1024;
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

pub(crate) fn kind() -> Kind {
    Kind {
        startup_parameters: vec![],
        shorthand: None,
        kind_id: kind_id("presentation/browser-dom-mask"),
        kind_contract_revision: KindIdentity::from("conduit.browser/presentation-dom-mask@1"),
        inputs: vec![port(
            "presentation",
            PRESENTATION_VALUE_KIND,
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

pub(crate) fn offer() -> conduit_core::CapabilityOffer {
    BackOfferBuilder::new(
        kind(),
        Back {
            capability_id: CapabilityId::from("capability/browser-dom-mask"),
            execution_profile_id: ExecutionProfileId::from("browser/mask@1"),
            implementation_id: ImplementationId::from("implementation/browser-dom-mask"),
            artifact_id: ArtifactId::from("artifact/browser-runtime"),
            host_calls: vec![HostCallRequirement {
                contract_id: HostCallContractId::from(MASK_OPERATION),
                target_kind: Some(kind_id("presentation/browser-dom-mask")),
                maximum_in_flight: 1,
                maximum_input_bytes: MASK_BYTES,
                maximum_output_bytes: MASK_BYTES,
            }],
            resource_requirements: vec![resource_requirement(PRESENTATION_RESOURCE_CLASS, 1)],
            authority_requirements: vec![],
        },
    )
    .build()
}
