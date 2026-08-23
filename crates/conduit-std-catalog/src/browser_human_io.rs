//! Browser Host offers for portable human I/O contracts.

use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, ArtifactId, AuthorityContractId, AuthorityRequirement, CapabilityId,
    CapabilityLimits, CapabilityOffer, ExecutionProfileId, HostOperationContractId,
    HostOperationRequirement, ImplementationId, ImplementationOffer, KindContractRevision,
    PortDescriptor, PortDirection, PortTemporal,
};

use crate::browser_presentation_nucleus_offers;

pub const CAMERA_ACQUIRE_KIND: &str = "media/acquire-camera@1";
pub const MICROPHONE_ACQUIRE_KIND: &str = "media/acquire-microphone@1";
pub const CAMERA_REQUEST_KIND: &str = "media/camera-constraints@1";
pub const MICROPHONE_REQUEST_KIND: &str = "media/microphone-constraints@1";
pub const MEDIA_ACQUISITION_RESULT_KIND: &str = "media/acquisition-result@1";
pub const CAMERA_FRAME_KIND: &str = "media/camera-frame@1";
pub const MICROPHONE_FRAME_KIND: &str = "media/microphone-frame@1";
pub const CAMERA_RESOURCE_CLASS: &str = "conduit.resource/acquired-camera@1";
pub const MICROPHONE_RESOURCE_CLASS: &str = "conduit.resource/acquired-microphone@1";
pub const MEDIA_ACQUIRE_OPERATION: &str = "conduit.host/acquire-human-media@1";
pub const MEDIA_REQUEST_AUTHORITY: &str = "conduit.authority/request-human-media@1";
pub const MEDIA_USE_AUTHORITY: &str = "conduit.authority/use-human-media@1";
pub const BROWSER_MEDIA_PROFILE: &str = "browser/human-media@1";
pub const BROWSER_MEDIA_ARTIFACT: &str = "conduit-browser-runtime/human-media@1";

pub const MAXIMUM_MEDIA_REQUEST_BYTES: u32 = 256;
pub const MAXIMUM_MEDIA_RESULT_BYTES: u32 = 1024;
pub const MAXIMUM_MEDIA_QUEUE_ITEMS: u16 = 4;
pub const MAXIMUM_MEDIA_QUEUE_BYTES: u32 = 4 * MAXIMUM_MEDIA_RESULT_BYTES;

pub fn browser_media_acquisition_offers() -> Vec<CapabilityOffer> {
    vec![
        acquisition_offer(CAMERA_ACQUIRE_KIND, CAMERA_REQUEST_KIND),
        acquisition_offer(MICROPHONE_ACQUIRE_KIND, MICROPHONE_REQUEST_KIND),
    ]
}

fn acquisition_offer(kind: &str, request_kind: &str) -> CapabilityOffer {
    let operation = HostOperationContractId::from(MEDIA_ACQUIRE_OPERATION);
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from(alloc::format!("browser/{kind}-capability").as_str()),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from("conduit.std/human-media@1"),
        inputs: vec![PortDescriptor {
            port_id: port_id("request"),
            value_kind: kind_id(request_kind),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
        }],
        outputs: vec![PortDescriptor {
            port_id: port_id("result"),
            value_kind: kind_id(MEDIA_ACQUISITION_RESULT_KIND),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
        }],
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(BROWSER_MEDIA_PROFILE),
            implementation_id: ImplementationId::from(alloc::format!("browser/{kind}").as_str()),
            artifact_id: ArtifactId::from(BROWSER_MEDIA_ARTIFACT),
        },
        host_operations: vec![HostOperationRequirement {
            contract_id: operation.clone(),
            target_kind: Some(kind_id(kind)),
            maximum_in_flight: 1,
            maximum_input_bytes: MAXIMUM_MEDIA_REQUEST_BYTES,
            maximum_output_bytes: MAXIMUM_MEDIA_RESULT_BYTES,
        }],
        resource_requirements: vec![],
        authority_requirements: vec![AuthorityRequirement {
            contract_id: AuthorityContractId::from(MEDIA_REQUEST_AUTHORITY),
            host_operation_contract_id: operation,
            subject_kind: kind_id(kind),
        }],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: MAXIMUM_MEDIA_QUEUE_ITEMS,
            max_queue_bytes: MAXIMUM_MEDIA_QUEUE_BYTES,
        },
    }
}

/// All pre-acquisition human-facing offers. Presentation remains the portable
/// Presentation nucleus; DOM/canvas are not semantic contracts.
pub fn browser_human_io_offers() -> Vec<CapabilityOffer> {
    let mut offers = browser_presentation_nucleus_offers();
    offers.extend(browser_media_acquisition_offers());
    offers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acquisition_ports_and_all_limits_are_exact_and_finite() {
        for offer in browser_media_acquisition_offers() {
            assert_eq!(offer.inputs.len(), 1);
            assert_eq!(offer.inputs[0].port_id.as_str(), "request");
            assert_eq!(offer.outputs.len(), 1);
            assert_eq!(offer.outputs[0].port_id.as_str(), "result");
            assert_eq!(offer.authority_requirements.len(), 1);
            assert_eq!(offer.host_operations.len(), 1);
            assert!(offer.limits.max_active_instances > 0);
            assert!(offer.limits.max_queue_items > 0);
            assert!(offer.limits.max_queue_bytes > 0);
            assert!(offer.host_operations[0].maximum_input_bytes > 0);
            assert!(offer.host_operations[0].maximum_output_bytes > 0);
        }
    }

    #[test]
    fn browser_presentation_is_portable_and_contains_no_renderer_kind() {
        let offers = browser_human_io_offers();
        assert!(offers
            .iter()
            .any(|offer| offer.kind_id.as_str() == crate::TEXT_PRESENTATION_KIND));
        assert!(offers
            .iter()
            .any(|offer| offer.kind_id.as_str() == crate::GRAPHICS_RECT_KIND));
        assert!(!offers.iter().any(|offer| {
            let kind = offer.kind_id.as_str();
            kind.contains("dom") || kind.contains("canvas")
        }));
    }
}
