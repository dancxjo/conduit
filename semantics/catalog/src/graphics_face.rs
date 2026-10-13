//! Terminal manifestation of one already-resolved bounded graphics scene.

use super::{KindTerminalBehavior, StandardKindContract};
use alloc::{string::ToString, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, PortDescriptor, PortDirection, PortTemporal,
};

pub const GRAPHICS_FACE_KIND: &str = "face/graphics";
pub const GRAPHICS_FACE_REVISION: &str = "conduit.std/face-graphics@1";

pub fn graphics_face_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(GRAPHICS_FACE_KIND),
        plain_name: "Graphics face".to_string(),
        summary: "Manifest one already-resolved bounded graphics scene on an exact admitted host face surface.".to_string(),
        inputs: vec![PortDescriptor {
            port_id: port_id("scene"),
            value_kind: kind_id(conduit_face::GRAPHICS_SCENE_KIND),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        }],
        outputs: Vec::new(),
        configuration: crate::data_label_configuration(),
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 1,
            max_queue_bytes: conduit_face::MAX_GRAPHICS_SCENE_BYTES as u32,
        },
        terminal_behavior: KindTerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: true,
        pico_manifestation_honest: false,
        example: "scene: graphics/icon >> display: face/graphics(\"Rendered scene\")".to_string(),
    }
}

pub fn bitmap_face_contract() -> StandardKindContract {
    let definition = conduit_face::bitmap_face_definition();
    StandardKindContract {
        kind_id: definition.kind_id,
        plain_name: "Bitmap face".to_string(),
        summary: "Manifest one bounded gray8 bitmap on an exact admitted host face surface."
            .to_string(),
        inputs: definition.inputs,
        outputs: definition.outputs,
        configuration: crate::data_label_configuration(),
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: 1,
            max_queue_bytes: conduit_face::MAX_GRAY8_BITMAP_BYTES as u32,
        },
        terminal_behavior: KindTerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example:
            "bitmap: graphics/scalar-field-gray8 >> display: face/bitmap(\"Scalar field bitmap\")"
                .to_string(),
    }
}

#[cfg(feature = "plot-catalog")]
pub fn install_graphics_face_catalog(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_plot::KindSignature;
    let contract = graphics_face_contract();
    startup.insert(KindSignature {
        kind: GRAPHICS_FACE_KIND.to_string(),
        startup_parameters: crate::data_label_startup_signature(),
    })?;
    profile
        .insert_kind(contract.into_semantic_contract(GRAPHICS_FACE_REVISION))
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_manifestation_is_terminal_bounded_and_mechanism_free() {
        let contract = graphics_face_contract();
        assert_eq!(
            contract.inputs[0].value_kind.as_str(),
            conduit_face::GRAPHICS_SCENE_KIND
        );
        assert!(contract.outputs.is_empty());
        let rendered = alloc::format!("{contract:?}").to_ascii_lowercase();
        for forbidden in ["framebuffer", "dom", "window", "pixel", "css"] {
            assert!(!rendered.contains(forbidden));
        }
    }

    #[test]
    fn bitmap_manifestation_is_terminal_bounded_and_mechanism_free() {
        let contract = bitmap_face_contract();
        assert_eq!(
            contract.inputs[0].value_kind.as_str(),
            conduit_face::GRAY8_BITMAP_INFO_KIND
        );
        let rendered = alloc::format!("{contract:?}").to_ascii_lowercase();
        for forbidden in ["framebuffer", "dom", "window", "css"] {
            assert!(!rendered.contains(forbidden));
        }
    }
}
