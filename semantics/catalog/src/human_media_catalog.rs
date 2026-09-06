//! Portable camera and microphone acquisition/use catalog contracts.

use alloc::{string::ToString, vec};
use conduit_core::{
    kind_id, port_id, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
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
pub const MAXIMUM_MEDIA_REQUEST_BYTES: u32 = 256;
pub const MAXIMUM_MEDIA_RESULT_BYTES: u32 = 1024;
pub const MAXIMUM_MEDIA_QUEUE_ITEMS: u16 = 4;
pub const MAXIMUM_MEDIA_QUEUE_BYTES: u32 = 4 * MAXIMUM_MEDIA_RESULT_BYTES;
pub const MAXIMUM_MEDIA_VALUE_BYTES: u32 = 64 * 1024;

#[cfg(feature = "form-catalog")]
pub(crate) fn install_camera_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_form::{KindDefinition, KindSignature};

    for (kind, revision, inputs, outputs) in [
        (
            CAMERA_SOURCE_KIND,
            "conduit.std/camera-source@1",
            vec![],
            vec![PortDescriptor {
                port_id: port_id("frame"),
                value_kind: kind_id(CAMERA_FRAME_KIND),
                direction: PortDirection::Output,
                temporal: PortTemporal::Flow { closes: true },
            }],
        ),
        (
            CAMERA_FRAME_SINK_KIND,
            "conduit.std/camera-frame-sink@1",
            vec![PortDescriptor {
                port_id: port_id("frame"),
                value_kind: kind_id(CAMERA_FRAME_KIND),
                direction: PortDirection::Input,
                temporal: PortTemporal::Flow { closes: true },
            }],
            vec![],
        ),
    ] {
        startup.insert(KindSignature {
            kind: kind.into(),
            startup_parameters: vec![],
        })?;
        profile
            .insert(KindDefinition {
                kind_id: kind_id(kind),
                kind_contract_revision: KindContractRevision::from(revision),
                inputs,
                outputs,
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
}
