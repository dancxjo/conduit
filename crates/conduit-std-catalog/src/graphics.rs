//! Canonical finite graphics leaves shared by recursive presenters.

use super::{
    StandardConfigurationField, StandardConfigurationRule, StandardKindContract, TerminalBehavior,
};
use alloc::{string::ToString, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ConfigurationValue, ExecutionProfileId, HostOperationContractId, HostOperationRequirement,
    ImplementationId, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
};
use conduit_presentation::{
    PresentationIconKey, GRAPHICS_SCENE_KIND, MAX_GRAPHICS_SCENE_BYTES, MAX_GRAPHICS_TEXT_BYTES,
    MAX_LAYOUT_EXTENT, MAX_PRESENTATION_COMPOSITION_BYTES, PRESENTATION_COMPOSITION_KIND,
};

pub const GRAPHICS_RECT_KIND: &str = "graphics/rect";
pub const GRAPHICS_TEXT_KIND: &str = "graphics/text";
pub const GRAPHICS_ICON_KIND: &str = "graphics/icon";
pub const GRAPHICS_RECT_IMPLEMENTATION: &str = "std/graphics/rect-implementation@1";
pub const GRAPHICS_TEXT_IMPLEMENTATION: &str = "std/graphics/text-implementation@1";
pub const GRAPHICS_ICON_IMPLEMENTATION: &str = "std/graphics/icon-implementation@1";
pub const GRAPHICS_HOST_OPERATION: &str = "conduit.host/graphics-scene-transform@1";
pub const GRAPHICS_INPUT_PORT: &str = "input";
pub const GRAPHICS_OUTPUT_PORT: &str = "scene";
pub const GRAPHICS_X_KEY: &str = "x";
pub const GRAPHICS_Y_KEY: &str = "y";
pub const GRAPHICS_WIDTH_KEY: &str = "width";
pub const GRAPHICS_HEIGHT_KEY: &str = "height";
pub const CLIP_X_KEY: &str = "clip-x";
pub const CLIP_Y_KEY: &str = "clip-y";
pub const CLIP_WIDTH_KEY: &str = "clip-width";
pub const CLIP_HEIGHT_KEY: &str = "clip-height";
pub const PAINT_KEY: &str = "paint";
pub const STYLE_KEY: &str = "style";
pub const GRAPHICS_TEXT_KEY: &str = "text";
pub const GRAPHICS_ICON_KEY: &str = "icon";
const REVISION: &str = "conduit.std/graphics-scene@1";
const PROFILE: &str = "conduit.std/graphics-scene-kernel@1";
const ARTIFACT: &str = "conduit-std-host/graphics-scene@1";

pub fn graphics_rect_contract() -> StandardKindContract {
    contract(
        GRAPHICS_RECT_KIND,
        "Graphics rectangle",
        "Resolve one clipped fill or stroke rectangle from a semantic presentation composition.",
        [
            geometry_fields(),
            vec![
                one_of(
                    PAINT_KEY,
                    "background",
                    &["background", "foreground", "accent", "status"],
                ),
                one_of(STYLE_KEY, "fill", &["fill", "stroke"]),
            ],
        ]
        .concat(),
        PRESENTATION_COMPOSITION_KIND,
    )
}
pub fn graphics_text_contract() -> StandardKindContract {
    contract(
        GRAPHICS_TEXT_KIND,
        "Graphics resolved text",
        "Append bounded already-resolved text at exact clipped geometry.",
        [
            geometry_fields(),
            vec![
                one_of(
                    PAINT_KEY,
                    "foreground",
                    &["background", "foreground", "accent", "status"],
                ),
                text_field(GRAPHICS_TEXT_KEY, "ready"),
            ],
        ]
        .concat(),
        GRAPHICS_SCENE_KIND,
    )
}
pub fn graphics_icon_contract() -> StandardKindContract {
    contract(
        GRAPHICS_ICON_KIND,
        "Graphics resolved icon",
        "Append one canonical resolved icon key at exact clipped geometry.",
        [
            geometry_fields(),
            vec![
                one_of(
                    PAINT_KEY,
                    "accent",
                    &["background", "foreground", "accent", "status"],
                ),
                one_of(
                    GRAPHICS_ICON_KEY,
                    "conduit-generic-gear",
                    &PresentationIconKey::ALL.map(PresentationIconKey::as_str),
                ),
            ],
        ]
        .concat(),
        GRAPHICS_SCENE_KIND,
    )
}

pub fn graphics_rect_offer() -> CapabilityOffer {
    offer(graphics_rect_contract())
}
pub fn graphics_text_offer() -> CapabilityOffer {
    offer(graphics_text_contract())
}
pub fn graphics_icon_offer() -> CapabilityOffer {
    offer(graphics_icon_contract())
}
pub fn graphics_offer_for(kind: &str) -> Option<CapabilityOffer> {
    Some(match kind {
        GRAPHICS_RECT_KIND => graphics_rect_offer(),
        GRAPHICS_TEXT_KIND => graphics_text_offer(),
        GRAPHICS_ICON_KIND => graphics_icon_offer(),
        _ => return None,
    })
}

fn contract(
    kind: &str,
    name: &str,
    summary: &str,
    configuration: Vec<StandardConfigurationField>,
    input_kind: &str,
) -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(kind),
        plain_name: name.to_string(),
        summary: summary.to_string(),
        inputs: vec![port(GRAPHICS_INPUT_PORT, input_kind, PortDirection::Input)],
        outputs: vec![port(
            GRAPHICS_OUTPUT_PORT,
            GRAPHICS_SCENE_KIND,
            PortDirection::Output,
        )],
        configuration,
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 1,
            max_queue_bytes: MAX_GRAPHICS_SCENE_BYTES.max(MAX_PRESENTATION_COMPOSITION_BYTES)
                as u32,
        },
        terminal_behavior:
            TerminalBehavior::EmitsOneDecisionOrCompletesWhenDecisionBecomesImpossible,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: format_example(kind),
    }
}
fn offer(contract: StandardKindContract) -> CapabilityOffer {
    let kind = contract.kind_id.as_str();
    CapabilityOffer {
        startup_parameters: super::functional_face::startup_face(&contract.configuration),
        shorthand: None,
        capability_id: CapabilityId::from(alloc::format!("std/{kind}-capability@1").as_str()),
        kind_id: contract.kind_id.clone(),
        kind_contract_revision: KindContractRevision::from(REVISION),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(PROFILE),
            implementation_id: ImplementationId::from(match kind {
                GRAPHICS_RECT_KIND => GRAPHICS_RECT_IMPLEMENTATION,
                GRAPHICS_TEXT_KIND => GRAPHICS_TEXT_IMPLEMENTATION,
                GRAPHICS_ICON_KIND => GRAPHICS_ICON_IMPLEMENTATION,
                _ => unreachable!(),
            }),
            artifact_id: ArtifactId::from(ARTIFACT),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(GRAPHICS_HOST_OPERATION),
            target_kind: Some(contract.kind_id),
            maximum_in_flight: 1,
            maximum_input_bytes: MAX_PRESENTATION_COMPOSITION_BYTES as u32,
            maximum_output_bytes: MAX_GRAPHICS_SCENE_BYTES as u32,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}
fn port(name: &str, value_kind: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal: PortTemporal::Value,
    }
}
fn geometry_fields() -> Vec<StandardConfigurationField> {
    vec![
        u16_field(GRAPHICS_X_KEY, 8, 0),
        u16_field(GRAPHICS_Y_KEY, 8, 0),
        u16_field(GRAPHICS_WIDTH_KEY, 120, 1),
        u16_field(GRAPHICS_HEIGHT_KEY, 40, 1),
        u16_field(CLIP_X_KEY, 0, 0),
        u16_field(CLIP_Y_KEY, 0, 0),
        u16_field(CLIP_WIDTH_KEY, 960, 1),
        u16_field(CLIP_HEIGHT_KEY, 540, 1),
    ]
}
fn u16_field(key: &str, default: u64, minimum: u64) -> StandardConfigurationField {
    StandardConfigurationField {
        key: key.to_string(),
        default_value: ConfigurationValue::U64(default),
        rule: StandardConfigurationRule::U64Range {
            minimum,
            maximum: u64::from(MAX_LAYOUT_EXTENT),
        },
    }
}
fn one_of(key: &str, default: &str, values: &[&str]) -> StandardConfigurationField {
    StandardConfigurationField {
        key: key.to_string(),
        default_value: ConfigurationValue::Text(default.into()),
        rule: StandardConfigurationRule::TextOneOf {
            values: values.iter().map(|value| (*value).to_string()).collect(),
        },
    }
}
fn text_field(key: &str, default: &str) -> StandardConfigurationField {
    StandardConfigurationField {
        key: key.to_string(),
        default_value: ConfigurationValue::Text(default.into()),
        rule: StandardConfigurationRule::TextBytes {
            maximum: MAX_GRAPHICS_TEXT_BYTES as u32,
        },
    }
}
fn format_example(kind: &str) -> alloc::string::String {
    alloc::format!("leaf: {kind}(x = 8, y = 8, width = 120, height = 40, clip-x = 0, clip-y = 0, clip-width = 960, clip-height = 540)")
}

#[cfg(feature = "form-catalog")]
pub fn install_graphics_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_form::{
        ConfigurationField, ConfigurationRule, KindDefinition, KindSignature,
        StartupParameterSignature,
    };
    for contract in [
        graphics_rect_contract(),
        graphics_text_contract(),
        graphics_icon_contract(),
    ] {
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().to_string(),
            startup_parameters: contract
                .configuration
                .iter()
                .map(|field| StartupParameterSignature {
                    name: field.key.clone(),
                    value_type: match field.default_value {
                        ConfigurationValue::Text(_) => "Text",
                        _ => "Count",
                    }
                    .to_string(),
                    default: Some(match &field.default_value {
                        ConfigurationValue::Text(value) => alloc::format!("{value:?}"),
                        ConfigurationValue::U64(value) => value.to_string(),
                        _ => unreachable!(),
                    }),
                })
                .collect(),
        })?;
        let configuration = contract
            .configuration
            .into_iter()
            .map(|field| ConfigurationField {
                key: field.key,
                default_value: field.default_value,
                validation: match field.rule {
                    StandardConfigurationRule::U64Range { minimum, maximum } => {
                        ConfigurationRule::U64Range { minimum, maximum }
                    }
                    StandardConfigurationRule::TextBytes { maximum } => {
                        ConfigurationRule::TextBytes { maximum }
                    }
                    StandardConfigurationRule::TextOneOf { values } => {
                        ConfigurationRule::TextOneOf { values }
                    }
                    _ => unreachable!(),
                },
            })
            .collect();
        profile
            .insert(KindDefinition {
                kind_id: contract.kind_id,
                kind_contract_revision: KindContractRevision::from(REVISION),
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration,
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reduced_family_is_exact_and_toolkit_free() {
        let contracts = [
            graphics_rect_contract(),
            graphics_text_contract(),
            graphics_icon_contract(),
        ];
        assert_eq!(contracts.len(), 3);
        for contract in contracts {
            let rendered = alloc::format!("{contract:?}");
            for forbidden in [
                "graphics/line",
                "graphics/path",
                "graphics/clip",
                "pixel",
                "framebuffer",
                "css",
                "svg",
            ] {
                assert!(!rendered.to_ascii_lowercase().contains(forbidden));
            }
            assert!(graphics_offer_for(contract.kind_id.as_str()).is_some());
        }
    }
}
