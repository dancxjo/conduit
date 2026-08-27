//! Browser Host offers for portable human I/O contracts.

use alloc::{string::ToString, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, resource_requirement, ArtifactId, AuthorityContractId, AuthorityRequirement,
    CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId, HostOperationContractId,
    HostOperationRequirement, ImplementationId, ImplementationOffer, KindContractRevision,
    PortDescriptor, PortDirection, PortTemporal,
};

pub const CAMERA_ACQUIRE_KIND: &str = "media/acquire-camera@1";
pub const MICROPHONE_ACQUIRE_KIND: &str = "media/acquire-microphone@1";
pub const CAMERA_REQUEST_KIND: &str = "media/camera-constraints@1";
pub const MICROPHONE_REQUEST_KIND: &str = "media/microphone-constraints@1";
pub const MEDIA_ACQUISITION_RESULT_KIND: &str = "media/acquisition-result@1";
pub const CAMERA_FRAME_KIND: &str = "media/camera-frame@1";
pub const MICROPHONE_FRAME_KIND: &str = "media/microphone-frame@1";
pub const CAMERA_SOURCE_KIND: &str = "media/camera";
pub const CAMERA_FRAME_SINK_KIND: &str = "media/frame-sink";
pub const CAMERA_RESOURCE_CLASS: &str = "conduit.resource/acquired-camera@1";
pub const MICROPHONE_RESOURCE_CLASS: &str = "conduit.resource/acquired-microphone@1";
pub const MEDIA_ACQUIRE_OPERATION: &str = "conduit.host/acquire-human-media@1";
pub const MEDIA_USE_OPERATION: &str = "conduit.host/use-human-media@1";
pub const MEDIA_REQUEST_AUTHORITY: &str = "conduit.authority/request-human-media@1";
pub const MEDIA_USE_AUTHORITY: &str = "conduit.authority/use-human-media@1";
pub const BROWSER_MEDIA_PROFILE: &str = "browser/human-media@1";
pub const BROWSER_MEDIA_ARTIFACT: &str = "conduit-browser-runtime/human-media@1";

pub const MAXIMUM_MEDIA_REQUEST_BYTES: u32 = 256;
pub const MAXIMUM_MEDIA_RESULT_BYTES: u32 = 1024;
pub const MAXIMUM_MEDIA_QUEUE_ITEMS: u16 = 4;
pub const MAXIMUM_MEDIA_QUEUE_BYTES: u32 = 4 * MAXIMUM_MEDIA_RESULT_BYTES;
pub const MAXIMUM_MEDIA_VALUE_BYTES: u32 = 64 * 1024;

pub fn browser_media_acquisition_offers() -> Vec<CapabilityOffer> {
    vec![
        acquisition_offer(CAMERA_ACQUIRE_KIND, CAMERA_REQUEST_KIND),
        acquisition_offer(MICROPHONE_ACQUIRE_KIND, MICROPHONE_REQUEST_KIND),
    ]
}

/// Semantic camera source made available only with post-acquisition resource
/// truth. Merely having a browser media API never installs this offer.
pub fn acquired_camera_source_offer() -> CapabilityOffer {
    let operation = HostOperationContractId::from(MEDIA_USE_OPERATION);
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from("browser/acquired-camera-source@1"),
        kind_id: kind_id(CAMERA_SOURCE_KIND),
        kind_contract_revision: KindContractRevision::from("conduit.std/camera-source@1"),
        inputs: vec![],
        outputs: vec![PortDescriptor {
            port_id: port_id("frame"),
            value_kind: kind_id(CAMERA_FRAME_KIND),
            direction: PortDirection::Output,
            temporal: PortTemporal::Flow { closes: true },
        }],
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(BROWSER_MEDIA_PROFILE),
            implementation_id: ImplementationId::from("browser/acquired-camera-source@1"),
            artifact_id: ArtifactId::from(BROWSER_MEDIA_ARTIFACT),
        },
        host_operations: vec![HostOperationRequirement {
            contract_id: operation.clone(),
            target_kind: Some(kind_id(CAMERA_FRAME_KIND)),
            maximum_in_flight: 1,
            maximum_input_bytes: 0,
            maximum_output_bytes: MAXIMUM_MEDIA_VALUE_BYTES,
        }],
        resource_requirements: vec![resource_requirement(CAMERA_RESOURCE_CLASS, 1)],
        authority_requirements: vec![AuthorityRequirement {
            contract_id: AuthorityContractId::from(MEDIA_USE_AUTHORITY),
            host_operation_contract_id: operation,
            subject_kind: kind_id(CAMERA_FRAME_KIND),
        }],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_MEDIA_VALUE_BYTES,
        },
    }
}

pub fn browser_camera_frame_sink_offer() -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from("browser/camera-frame-sink@1"),
        kind_id: kind_id(CAMERA_FRAME_SINK_KIND),
        kind_contract_revision: KindContractRevision::from("conduit.std/camera-frame-sink@1"),
        inputs: vec![PortDescriptor {
            port_id: port_id("frame"),
            value_kind: kind_id(CAMERA_FRAME_KIND),
            direction: PortDirection::Input,
            temporal: PortTemporal::Flow { closes: true },
        }],
        outputs: vec![],
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(
                "conduit.std/camera-frame-sink-kernel@1",
            ),
            implementation_id: ImplementationId::from("std/kernel-camera-frame-sink@1"),
            artifact_id: ArtifactId::from(BROWSER_MEDIA_ARTIFACT),
        },
        host_operations: vec![],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_MEDIA_VALUE_BYTES,
        },
    }
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

#[cfg(feature = "form-catalog")]
pub fn install_human_media_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_form::{KindDefinition, KindSignature};

    for offer in [
        acquired_camera_source_offer(),
        browser_camera_frame_sink_offer(),
    ] {
        startup.insert(KindSignature {
            kind: offer.kind_id.as_str().into(),
            startup_parameters: vec![],
        })?;
        profile
            .insert(KindDefinition {
                kind_id: offer.kind_id,
                kind_contract_revision: offer.kind_contract_revision,
                inputs: offer.inputs,
                outputs: offer.outputs,
                configuration: vec![],
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
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

    #[cfg(feature = "form-catalog")]
    #[test]
    fn camera_summary_form_is_browser_neutral_and_has_one_exact_typed_cord() {
        let source = include_str!("../../../examples/camera-summary.conduit");
        let lower = source.to_ascii_lowercase();
        for forbidden in [
            "browser",
            "dom",
            "canvas",
            "device",
            "permission",
            "transport",
            "socket",
            "address",
            "url",
            "host",
        ] {
            assert!(!lower.contains(forbidden), "Form contains {forbidden}");
        }
        let mut startup = conduit_form::StartupCatalog::new();
        let mut profile = conduit_form::ProfileCatalog::new();
        install_human_media_catalogs(&mut startup, &mut profile).unwrap();
        let checked = conduit_form::check_syntax_document(
            &conduit_form::parse_syntax_document(source),
            &startup,
        )
        .unwrap();
        let expanded =
            conduit_form::expand_canonical_form(&checked, "camera-summary", &profile).unwrap();
        assert_eq!(expanded.gears.len(), 2);
        assert_eq!(expanded.connections.len(), 1);
        assert_eq!(
            expanded.connections[0].value_kind.as_str(),
            CAMERA_FRAME_KIND
        );
        assert_eq!(expanded.connections[0].source_port_id.as_str(), "frame");
        assert_eq!(expanded.connections[0].sink_port_id.as_str(), "frame");
    }
}
