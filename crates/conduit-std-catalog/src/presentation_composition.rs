//! Small renderer-neutral presentation composition family.

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
    PresentationIconKey, MAX_COMPOSITION_NAME_BYTES, MAX_PRESENTATION_COMPOSITION_BYTES,
    PRESENTATION_COMPOSITION_KIND,
};

pub const PRESENTATION_ICON_KIND: &str = "presentation/icon";
pub const PRESENTATION_FRAME_KIND: &str = "presentation/frame";
pub const PRESENTATION_BADGE_KIND: &str = "presentation/badge";
pub const PRESENTATION_ICON_IMPLEMENTATION: &str = "std/presentation/icon-implementation@1";
pub const PRESENTATION_FRAME_IMPLEMENTATION: &str = "std/presentation/frame-implementation@1";
pub const PRESENTATION_BADGE_IMPLEMENTATION: &str = "std/presentation/badge-implementation@1";
pub const PRESENTATION_INPUT_PORT: &str = "content";
pub const PRESENTATION_OUTPUT_PORT: &str = "presented";
pub const ICON_KEY: &str = "icon";
pub const ROLE_KEY: &str = "role";
pub const STATE_KEY: &str = "state";
pub const ACCESSIBILITY_NAME_KEY: &str = "accessibility-name";
pub const PRESENTATION_COMPOSITION_HOST_OPERATION: &str =
    "conduit.host/presentation-composition-transform@1";

const REVISION: &str = "conduit.std/presentation-composition@1";
const PROFILE: &str = "conduit.std/presentation-composition-kernel@1";
const ARTIFACT: &str = "conduit-std-host/presentation-composition@1";

pub fn presentation_icon_contract() -> StandardKindContract {
    contract(
        PRESENTATION_ICON_KIND,
        "Presentation icon",
        "Resolve one authoritative local icon identity with an exact accessible name.",
        vec![
            StandardConfigurationField {
                key: ICON_KEY.to_string(),
                default_value: ConfigurationValue::Text("conduit-generic-gear".into()),
                rule: StandardConfigurationRule::TextOneOf {
                    values: PresentationIconKey::ALL
                        .iter()
                        .map(|value| value.as_str().to_string())
                        .collect(),
                },
            },
            name_field("generic Gear; icon metadata missing"),
        ],
        false,
        "icon: presentation/icon(icon = \"presentation\", accessibility-name = \"Patchbay\")",
    )
}

pub fn presentation_frame_contract() -> StandardKindContract {
    contract(
        PRESENTATION_FRAME_KIND,
        "Presentation frame",
        "Group bounded content with one renderer-neutral semantic frame role.",
        vec![text_field(ROLE_KEY, "panel"), name_field("Gear Face")],
        true,
        "frame: presentation/frame(role = \"panel\", accessibility-name = \"Gear Face\")",
    )
}

pub fn presentation_badge_contract() -> StandardKindContract {
    contract(
        PRESENTATION_BADGE_KIND,
        "Presentation badge",
        "Annotate bounded content with one compact semantic status.",
        vec![text_field(STATE_KEY, "ready"), name_field("ready")],
        true,
        "badge: presentation/badge(state = \"ready\", accessibility-name = \"ready\")",
    )
}

pub fn presentation_icon_offer() -> CapabilityOffer {
    offer(presentation_icon_contract())
}
pub fn presentation_frame_offer() -> CapabilityOffer {
    offer(presentation_frame_contract())
}
pub fn presentation_badge_offer() -> CapabilityOffer {
    offer(presentation_badge_contract())
}

pub fn presentation_composition_offer_for(kind: &str) -> Option<CapabilityOffer> {
    Some(match kind {
        PRESENTATION_ICON_KIND => presentation_icon_offer(),
        PRESENTATION_FRAME_KIND => presentation_frame_offer(),
        PRESENTATION_BADGE_KIND => presentation_badge_offer(),
        _ => return None,
    })
}

fn contract(
    kind: &str,
    plain_name: &str,
    summary: &str,
    configuration: Vec<StandardConfigurationField>,
    has_input: bool,
    example: &str,
) -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(kind),
        plain_name: plain_name.to_string(),
        summary: summary.to_string(),
        inputs: if has_input {
            vec![port(PRESENTATION_INPUT_PORT, PortDirection::Input)]
        } else {
            Vec::new()
        },
        outputs: vec![port(PRESENTATION_OUTPUT_PORT, PortDirection::Output)],
        configuration,
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 1,
            max_queue_bytes: MAX_PRESENTATION_COMPOSITION_BYTES as u32,
        },
        terminal_behavior: if has_input {
            TerminalBehavior::EmitsOneDecisionOrCompletesWhenDecisionBecomesImpossible
        } else {
            TerminalBehavior::EmitsOnce
        },
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: example.to_string(),
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
                PRESENTATION_ICON_KIND => PRESENTATION_ICON_IMPLEMENTATION,
                PRESENTATION_FRAME_KIND => PRESENTATION_FRAME_IMPLEMENTATION,
                PRESENTATION_BADGE_KIND => PRESENTATION_BADGE_IMPLEMENTATION,
                _ => unreachable!(),
            }),
            artifact_id: ArtifactId::from(ARTIFACT),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: if kind == PRESENTATION_ICON_KIND {
            Vec::new()
        } else {
            vec![HostOperationRequirement {
                contract_id: HostOperationContractId::from(PRESENTATION_COMPOSITION_HOST_OPERATION),
                target_kind: Some(contract.kind_id),
                maximum_in_flight: 1,
                maximum_input_bytes: MAX_PRESENTATION_COMPOSITION_BYTES as u32,
                maximum_output_bytes: MAX_PRESENTATION_COMPOSITION_BYTES as u32,
            }]
        },
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

fn port(name: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(PRESENTATION_COMPOSITION_KIND),
        direction,
        temporal: PortTemporal::Value,
    }
}

fn text_field(key: &str, default: &str) -> StandardConfigurationField {
    StandardConfigurationField {
        key: key.to_string(),
        default_value: ConfigurationValue::Text(default.into()),
        rule: StandardConfigurationRule::TextBytes {
            maximum: MAX_COMPOSITION_NAME_BYTES as u32,
        },
    }
}

fn name_field(default: &str) -> StandardConfigurationField {
    text_field(ACCESSIBILITY_NAME_KEY, default)
}

#[cfg(feature = "form-catalog")]
pub fn install_presentation_composition_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_form::{
        ConfigurationField, ConfigurationRule, KindDefinition, KindSignature,
        StartupParameterSignature,
    };
    for contract in [
        presentation_icon_contract(),
        presentation_frame_contract(),
        presentation_badge_contract(),
    ] {
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().to_string(),
            startup_parameters: contract
                .configuration
                .iter()
                .map(|field| StartupParameterSignature {
                    name: field.key.clone(),
                    value_type: "Text".to_string(),
                    default: match &field.default_value {
                        ConfigurationValue::Text(value) => Some(alloc::format!("{value:?}")),
                        _ => unreachable!(),
                    },
                })
                .collect(),
        })?;
        profile
            .insert(KindDefinition {
                kind_id: contract.kind_id,
                kind_contract_revision: KindContractRevision::from(REVISION),
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration: contract
                    .configuration
                    .into_iter()
                    .map(|field| ConfigurationField {
                        key: field.key,
                        default_value: field.default_value,
                        validation: match field.rule {
                            StandardConfigurationRule::TextBytes { maximum } => {
                                ConfigurationRule::TextBytes { maximum }
                            }
                            StandardConfigurationRule::TextOneOf { values } => {
                                ConfigurationRule::TextOneOf { values }
                            }
                            _ => unreachable!(),
                        },
                    })
                    .collect(),
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TEXT_PRESENTATION_KIND;

    #[test]
    fn family_reuses_text_and_has_no_renderer_mechanics() {
        let contracts = [
            presentation_icon_contract(),
            presentation_frame_contract(),
            presentation_badge_contract(),
        ];
        assert!(contracts.iter().all(|contract| {
            contract
                .inputs
                .iter()
                .chain(&contract.outputs)
                .all(|port| port.value_kind.as_str() == PRESENTATION_COMPOSITION_KIND)
        }));
        assert!(!contracts
            .iter()
            .any(|contract| contract.kind_id.as_str() == TEXT_PRESENTATION_KIND));
        assert_eq!(
            super::super::text_presentation_contract().kind_id.as_str(),
            TEXT_PRESENTATION_KIND
        );
    }
}
