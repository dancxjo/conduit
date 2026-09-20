//! Portable camera and microphone acquisition/use catalog contracts.

use alloc::{string::ToString, vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, KindContractRevision, PortDescriptor, PortDirection,
    PortTemporal, SemanticCapabilityContract,
};

pub const CAMERA_ACQUIRE_KIND: &str = "media/acquire-camera@1";
pub const MICROPHONE_ACQUIRE_KIND: &str = "media/acquire-microphone@1";
pub const CAMERA_REQUEST_KIND: &str = "media/camera-constraints@1";
pub const MICROPHONE_REQUEST_KIND: &str = "media/microphone-constraints@1";
pub const MEDIA_ACQUISITION_RESULT_KIND: &str = "media/acquisition-result@1";
pub const CAMERA_FRAME_KIND: &str = "media/camera-frame@1";
pub const MICROPHONE_FRAME_KIND: &str = "media/microphone-frame@1";
pub const MICROPHONE_CLIP_SOURCE_KIND: &str = "media/capture-microphone-clip";
pub const MICROPHONE_CLIP_SOURCE_REVISION: &str = "conduit.std/microphone-clip-source@1";
pub const CAMERA_SOURCE_KIND: &str = "media/camera";
pub const CAMERA_FRAME_SINK_KIND: &str = "media/frame-sink";
pub const CAMERA_RESOURCE_CLASS: &str = "conduit.resource/acquired-camera@1";
pub const MICROPHONE_RESOURCE_CLASS: &str = "conduit.resource/acquired-microphone@1";
pub const MEDIA_ACQUIRE_OPERATION: &str = "conduit.host/acquire-human-media@1";
pub const MEDIA_USE_OPERATION: &str = "conduit.host/use-human-media@1";
pub const MEDIA_REQUEST_AUTHORITY: &str = "conduit.authority/request-human-media@1";
pub const MEDIA_USE_AUTHORITY: &str = "conduit.authority/use-human-media@1";
pub const MAXIMUM_MEDIA_REQUEST_BYTES: u32 = 256;
pub const MAXIMUM_MEDIA_RESULT_BYTES: u32 = 1024;
pub const MAXIMUM_MEDIA_QUEUE_ITEMS: u16 = 4;
pub const MAXIMUM_MEDIA_QUEUE_BYTES: u32 = 4 * MAXIMUM_MEDIA_RESULT_BYTES;
pub const MAXIMUM_MEDIA_VALUE_BYTES: u32 = 64 * 1024;

pub fn media_acquisition_semantic_contract(kind: &str) -> Option<SemanticCapabilityContract> {
    let request_kind = match kind {
        CAMERA_ACQUIRE_KIND => CAMERA_REQUEST_KIND,
        MICROPHONE_ACQUIRE_KIND => MICROPHONE_REQUEST_KIND,
        _ => return None,
    };
    Some(SemanticCapabilityContract {
        startup_parameters: vec![],
        shorthand: None,
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
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: MAXIMUM_MEDIA_QUEUE_ITEMS,
            max_queue_bytes: MAXIMUM_MEDIA_QUEUE_BYTES,
        },
    })
}

pub fn camera_source_semantic_contract() -> SemanticCapabilityContract {
    SemanticCapabilityContract {
        startup_parameters: vec![],
        shorthand: None,
        kind_id: kind_id(CAMERA_SOURCE_KIND),
        kind_contract_revision: KindContractRevision::from("conduit.std/camera-source@1"),
        inputs: vec![],
        outputs: vec![camera_frame_port(PortDirection::Output)],
        limits: camera_limits(),
    }
}

pub fn camera_frame_sink_semantic_contract() -> SemanticCapabilityContract {
    SemanticCapabilityContract {
        startup_parameters: vec![],
        shorthand: None,
        kind_id: kind_id(CAMERA_FRAME_SINK_KIND),
        kind_contract_revision: KindContractRevision::from("conduit.std/camera-frame-sink@1"),
        inputs: vec![camera_frame_port(PortDirection::Input)],
        outputs: vec![],
        limits: camera_limits(),
    }
}

fn camera_frame_port(direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id("frame"),
        value_kind: kind_id(CAMERA_FRAME_KIND),
        direction,
        temporal: PortTemporal::Flow { closes: true },
    }
}

fn camera_limits() -> CapabilityLimits {
    CapabilityLimits {
        max_active_instances: 1,
        max_queue_items: 1,
        max_queue_bytes: MAXIMUM_MEDIA_VALUE_BYTES,
    }
}

pub fn microphone_clip_source_semantic_contract() -> SemanticCapabilityContract {
    SemanticCapabilityContract {
        startup_parameters: vec![],
        shorthand: None,
        kind_id: kind_id(MICROPHONE_CLIP_SOURCE_KIND),
        kind_contract_revision: KindContractRevision::from(MICROPHONE_CLIP_SOURCE_REVISION),
        inputs: vec![PortDescriptor {
            port_id: port_id("request"),
            value_kind: kind_id(conduit_text::TEXT_VALUE_KIND),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
        }],
        outputs: vec![PortDescriptor {
            port_id: port_id("clip"),
            value_kind: kind_id(conduit_audio::AUDIO_PCM_CLIP_INFO_ID),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
        }],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32,
        },
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_microphone_clip_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_form::{KindDefinition, KindSignature};

    startup.insert(KindSignature {
        kind: MICROPHONE_CLIP_SOURCE_KIND.into(),
        startup_parameters: vec![],
    })?;
    let contract = microphone_clip_source_semantic_contract();
    profile
        .insert(KindDefinition {
            kind_id: contract.kind_id,
            kind_contract_revision: contract.kind_contract_revision,
            inputs: contract.inputs,
            outputs: contract.outputs,
            configuration: vec![],
        })
        .map_err(|error| error.to_string())
}

#[cfg(feature = "form-catalog")]
pub(crate) fn install_camera_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_form::{KindDefinition, KindSignature};

    for contract in [
        camera_source_semantic_contract(),
        camera_frame_sink_semantic_contract(),
    ] {
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().into(),
            startup_parameters: vec![],
        })?;
        profile
            .insert(KindDefinition {
                kind_id: contract.kind_id,
                kind_contract_revision: contract.kind_contract_revision,
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration: vec![],
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(all(test, feature = "form-catalog"))]
mod tests {
    use super::*;

    #[test]
    fn camera_summary_form_is_browser_neutral_and_has_one_exact_typed_cord() {
        let source = include_str!("../../../forms/camera-summary/main.conduit");
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
        install_camera_catalogs(&mut startup, &mut profile).unwrap();
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

    #[test]
    fn microphone_clip_capture_is_portable_and_explicitly_triggered() {
        let mut startup = conduit_form::StartupCatalog::new();
        let mut profile = conduit_form::ProfileCatalog::new();
        conduit_text::install_text_catalogs(&mut startup, &mut profile).unwrap();
        install_microphone_clip_catalogs(&mut startup, &mut profile).unwrap();
        let checked = conduit_form::check_syntax_document(
            &conduit_form::parse_syntax_document(
                "form capture {\n microphone: media/capture-microphone-clip\n \"capture\" > microphone.request\n}\n",
            ),
            &startup,
        )
        .unwrap();
        let expanded = conduit_form::expand_canonical_form(&checked, "capture", &profile).unwrap();
        assert_eq!(expanded.gears.len(), 2);
        let microphone = expanded
            .gears
            .iter()
            .find(|gear| gear.kind_id.as_str() == MICROPHONE_CLIP_SOURCE_KIND)
            .unwrap();
        assert_eq!(microphone.inputs[0].port_id.as_str(), "request");
        assert_eq!(microphone.outputs[0].port_id.as_str(), "clip");
        assert_eq!(
            microphone.outputs[0].value_kind.as_str(),
            conduit_audio::AUDIO_PCM_CLIP_INFO_ID
        );
    }
}
