//! Terminal manifestation of one already-resolved bounded graphics scene.

use super::{StandardKindContract, TerminalBehavior};
use alloc::{string::ToString, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, present_host_operation_requirement, resource_requirement, ArtifactId,
    CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId, ImplementationId,
    KindContractRevision, PortDescriptor, PortDirection, PortTemporal, PRESENTATION_RESOURCE_CLASS,
};

pub const GRAPHICS_PRESENTATION_KIND: &str = "presentation/graphics";
pub const GRAPHICS_PRESENTATION_REVISION: &str = "conduit.std/presentation-graphics@1";
pub const GRAPHICS_PRESENTATION_PROFILE: &str = "conduit.std/presentation-graphics-kernel-hosted@1";
pub const GRAPHICS_PRESENTATION_IMPLEMENTATION: &str = "std/kernel-presentation-graphics@1";
pub const GRAPHICS_PRESENTATION_ARTIFACT: &str = "conduit-std-host/presentation-graphics@1";
pub const GRAPHICS_PRESENTATION_HOST_OPERATION: &str = "conduit.host/present@1";
pub const BITMAP_PRESENTATION_KIND: &str = "presentation/bitmap";
pub const BITMAP_PRESENTATION_REVISION: &str = "conduit.presentation/bitmap@1";
pub const BITMAP_PRESENTATION_PROFILE: &str = "conduit.std/presentation-bitmap-gray8@1";
pub const BITMAP_PRESENTATION_IMPLEMENTATION: &str = "std/kernel-presentation-bitmap@1";
pub const BITMAP_PRESENTATION_ARTIFACT: &str = "conduit-std-host/presentation-bitmap@1";

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
        }],
        outputs: Vec::new(),
        configuration: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 1,
            max_queue_bytes: conduit_presentation::MAX_GRAPHICS_SCENE_BYTES as u32,
        },
        terminal_behavior: TerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: true,
        pico_manifestation_honest: false,
        example: "scene: graphics/icon > display: presentation/graphics".to_string(),
    }
}

pub fn graphics_presentation_offer() -> CapabilityOffer {
    let contract = graphics_presentation_contract();
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("presentation-graphics-v1"),
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(GRAPHICS_PRESENTATION_REVISION),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(GRAPHICS_PRESENTATION_PROFILE),
            implementation_id: ImplementationId::from(GRAPHICS_PRESENTATION_IMPLEMENTATION),
            artifact_id: ArtifactId::from(GRAPHICS_PRESENTATION_ARTIFACT),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![present_host_operation_requirement(
            kind_id("presentation/graphics-scene"),
            conduit_presentation::MAX_GRAPHICS_SCENE_BYTES as u32,
        )],
        resource_requirements: vec![resource_requirement(PRESENTATION_RESOURCE_CLASS, 1)],
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

pub fn bitmap_presentation_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(BITMAP_PRESENTATION_KIND),
        plain_name: "Bitmap presentation".to_string(),
        summary:
            "Manifest one bounded gray8 bitmap on an exact admitted host presentation surface."
                .to_string(),
        inputs: vec![PortDescriptor {
            port_id: port_id("bitmap"),
            value_kind: kind_id(conduit_presentation::GRAY8_BITMAP_INFO_KIND),
            direction: PortDirection::Input,
            temporal: PortTemporal::Flow { closes: true },
        }],
        outputs: Vec::new(),
        configuration: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: 1,
            max_queue_bytes: conduit_presentation::MAX_GRAY8_BITMAP_BYTES as u32,
        },
        terminal_behavior: TerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "bitmap: graphics/scalar-field-gray8 > display: presentation/bitmap".to_string(),
    }
}

pub fn bitmap_presentation_offer() -> CapabilityOffer {
    let contract = bitmap_presentation_contract();
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("presentation-bitmap-gray8-v1"),
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(BITMAP_PRESENTATION_REVISION),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(BITMAP_PRESENTATION_PROFILE),
            implementation_id: ImplementationId::from(BITMAP_PRESENTATION_IMPLEMENTATION),
            artifact_id: ArtifactId::from(BITMAP_PRESENTATION_ARTIFACT),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![present_host_operation_requirement(
            kind_id("presentation/bitmap-gray8"),
            conduit_presentation::MAX_GRAY8_BITMAP_BYTES as u32,
        )],
        resource_requirements: vec![resource_requirement(PRESENTATION_RESOURCE_CLASS, 1)],
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_graphics_presentation_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_form::{KindDefinition, KindSignature};
    for (kind, revision, contract) in [
        (
            GRAPHICS_PRESENTATION_KIND,
            GRAPHICS_PRESENTATION_REVISION,
            graphics_presentation_contract(),
        ),
        (
            BITMAP_PRESENTATION_KIND,
            BITMAP_PRESENTATION_REVISION,
            bitmap_presentation_contract(),
        ),
    ] {
        startup.insert(KindSignature {
            kind: kind.to_string(),
            startup_parameters: Vec::new(),
        })?;
        profile
            .insert(KindDefinition {
                kind_id: contract.kind_id,
                kind_contract_revision: KindContractRevision::from(revision),
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration: Vec::new(),
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_manifestation_is_terminal_bounded_and_mechanism_free() {
        let contract = graphics_presentation_contract();
        let offer = graphics_presentation_offer();
        assert_eq!(
            contract.inputs[0].value_kind.as_str(),
            conduit_presentation::GRAPHICS_SCENE_KIND
        );
        assert!(contract.outputs.is_empty());
        assert_eq!(offer.host_operations.len(), 1);
        assert_eq!(offer.resource_requirements.len(), 1);
        let rendered = alloc::format!("{contract:?}").to_ascii_lowercase();
        for forbidden in ["framebuffer", "dom", "window", "pixel", "css"] {
            assert!(!rendered.contains(forbidden));
        }
    }

    #[test]
    fn bitmap_manifestation_is_terminal_bounded_and_mechanism_free() {
        let contract = bitmap_presentation_contract();
        let offer = bitmap_presentation_offer();
        assert_eq!(
            contract.inputs[0].value_kind.as_str(),
            conduit_presentation::GRAY8_BITMAP_INFO_KIND
        );
        assert_eq!(offer.host_operations.len(), 1);
        let rendered = alloc::format!("{contract:?}").to_ascii_lowercase();
        for forbidden in ["framebuffer", "dom", "window", "css"] {
            assert!(!rendered.contains(forbidden));
        }
    }
}
