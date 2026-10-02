//! Terminal manifestation of one already-resolved bounded graphics scene.

use super::{KindTerminalBehavior, StandardKindContract};
use alloc::{string::ToString, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, PortDescriptor, PortDirection, PortTemporal,
};

pub const GRAPHICS_PRESENTATION_KIND: &str = "presentation/graphics";
pub const GRAPHICS_PRESENTATION_REVISION: &str = "conduit.std/presentation-graphics@1";

pub fn graphics_presentation_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(GRAPHICS_PRESENTATION_KIND),
        plain_name: "Graphics presentation".to_string(),
        summary: "Manifest one already-resolved bounded graphics scene on an exact admitted host presentation surface.".to_string(),
        inputs: vec![PortDescriptor {
            port_id: port_id("scene"),
            value_kind: kind_id(conduit_presentation::GRAPHICS_SCENE_KIND),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        }],
        outputs: Vec::new(),
        configuration: Default::default(),
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 1,
            max_queue_bytes: conduit_presentation::MAX_GRAPHICS_SCENE_BYTES as u32,
        },
        terminal_behavior: KindTerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: true,
        pico_manifestation_honest: false,
        example: "scene: graphics/icon >> display: presentation/graphics".to_string(),
    }
}

pub fn bitmap_presentation_contract() -> StandardKindContract {
    let definition = conduit_presentation::bitmap_presentation_definition();
    StandardKindContract {
        kind_id: definition.kind_id,
        plain_name: "Bitmap presentation".to_string(),
        summary:
            "Manifest one bounded gray8 bitmap on an exact admitted host presentation surface."
                .to_string(),
        inputs: definition.inputs,
        outputs: definition.outputs,
        configuration: Default::default(),
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: 1,
            max_queue_bytes: conduit_presentation::MAX_GRAY8_BITMAP_BYTES as u32,
        },
        terminal_behavior: KindTerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "bitmap: graphics/scalar-field-gray8 >> display: presentation/bitmap".to_string(),
    }
}

#[cfg(feature = "plot-catalog")]
pub fn install_graphics_presentation_catalog(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_plot::KindSignature;
    let contract = graphics_presentation_contract();
    startup.insert(KindSignature {
        kind: GRAPHICS_PRESENTATION_KIND.to_string(),
        startup_parameters: Vec::new(),
    })?;
    profile
        .insert_kind(contract.into_semantic_contract(GRAPHICS_PRESENTATION_REVISION))
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_manifestation_is_terminal_bounded_and_mechanism_free() {
        let contract = graphics_presentation_contract();
        assert_eq!(
            contract.inputs[0].value_kind.as_str(),
            conduit_presentation::GRAPHICS_SCENE_KIND
        );
        assert!(contract.outputs.is_empty());
        let rendered = alloc::format!("{contract:?}").to_ascii_lowercase();
        for forbidden in ["framebuffer", "dom", "window", "pixel", "css"] {
            assert!(!rendered.contains(forbidden));
        }
    }

    #[test]
    fn bitmap_manifestation_is_terminal_bounded_and_mechanism_free() {
        let contract = bitmap_presentation_contract();
        assert_eq!(
            contract.inputs[0].value_kind.as_str(),
            conduit_presentation::GRAY8_BITMAP_INFO_KIND
        );
        let rendered = alloc::format!("{contract:?}").to_ascii_lowercase();
        for forbidden in ["framebuffer", "dom", "window", "css"] {
            assert!(!rendered.contains(forbidden));
        }
    }
}
