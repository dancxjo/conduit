//! Small renderer-neutral face composition family.

use super::{
    KindConfigurationField, KindConfigurationRule, KindTerminalBehavior, StandardKindContract,
};
use alloc::{string::ToString, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, ConfigurationValue, PortDescriptor, PortDirection,
    PortTemporal,
};
use conduit_face::{
    FaceIconKey, FACE_COMPOSITION_KIND, MAX_COMPOSITION_NAME_BYTES, MAX_FACE_COMPOSITION_BYTES,
};

pub const FACE_ICON_KIND: &str = "face/icon";
pub const FACE_FRAME_KIND: &str = "face/frame";
pub const FACE_BADGE_KIND: &str = "face/badge";
pub const FACE_INPUT_PORT: &str = "content";
pub const FACE_OUTPUT_PORT: &str = "presented";
pub const ICON_KEY: &str = "icon";
pub const ROLE_KEY: &str = "role";
pub const STATE_KEY: &str = "state";
pub const FACE_COMPOSITION_CONTRACT_REVISION: &str = "conduit.std/face-composition@1";

pub fn face_icon_contract() -> StandardKindContract {
    contract(
        FACE_ICON_KIND,
        "Face icon",
        "Resolve one authoritative local icon identity with one exact human name.",
        vec![KindConfigurationField {
            key: ICON_KEY.to_string(),
            default_value: ConfigurationValue::Text("conduit-generic-gear".into()),
            rule: KindConfigurationRule::TextOneOf {
                values: FaceIconKey::ALL
                    .iter()
                    .map(|value| value.as_str().to_string())
                    .collect(),
            },
        }],
        false,
        "icon: face/icon(label = \"Patchbay\", icon = \"face\")",
    )
}

pub fn face_frame_contract() -> StandardKindContract {
    contract(
        FACE_FRAME_KIND,
        "Face frame",
        "Group bounded content with one renderer-neutral semantic frame role.",
        vec![text_field(ROLE_KEY, "panel")],
        true,
        "frame: face/frame(label = \"Gear Front\", role = \"panel\")",
    )
}

pub fn face_badge_contract() -> StandardKindContract {
    contract(
        FACE_BADGE_KIND,
        "Face badge",
        "Annotate bounded content with one compact semantic status.",
        vec![text_field(STATE_KEY, "ready")],
        true,
        "badge: face/badge(label = \"Readiness\", state = \"ready\")",
    )
}

pub fn face_composition_contract_for(kind: &str) -> Option<StandardKindContract> {
    Some(match kind {
        FACE_ICON_KIND => face_icon_contract(),
        FACE_FRAME_KIND => face_frame_contract(),
        FACE_BADGE_KIND => face_badge_contract(),
        _ => return None,
    })
}

fn contract(
    kind: &str,
    plain_name: &str,
    summary: &str,
    mut configuration: Vec<KindConfigurationField>,
    has_input: bool,
    example: &str,
) -> StandardKindContract {
    let mut label = crate::data_label_configuration().remove(0);
    label.rule = KindConfigurationRule::DataLabel {
        maximum: MAX_COMPOSITION_NAME_BYTES as u32,
    };
    configuration.insert(0, label);
    StandardKindContract {
        kind_id: kind_id(kind),
        plain_name: plain_name.to_string(),
        summary: summary.to_string(),
        inputs: if has_input {
            vec![port(FACE_INPUT_PORT, PortDirection::Input)]
        } else {
            Vec::new()
        },
        outputs: vec![port(FACE_OUTPUT_PORT, PortDirection::Output)],
        configuration,
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 1,
            max_queue_bytes: MAX_FACE_COMPOSITION_BYTES as u32,
        },
        terminal_behavior: if has_input {
            KindTerminalBehavior::EmitsOneDecisionOrCompletesWhenDecisionBecomesImpossible
        } else {
            KindTerminalBehavior::EmitsOnce
        },
        hosted_implementation_required: true,
        browser_manifestation_honest: true,
        pico_manifestation_honest: false,
        example: example.to_string(),
    }
}

fn port(name: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(FACE_COMPOSITION_KIND),
        direction,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    }
}

fn text_field(key: &str, default: &str) -> KindConfigurationField {
    KindConfigurationField {
        key: key.to_string(),
        default_value: ConfigurationValue::Text(default.into()),
        rule: KindConfigurationRule::TextBytes {
            maximum: MAX_COMPOSITION_NAME_BYTES as u32,
        },
    }
}

#[cfg(feature = "plot-catalog")]
pub fn install_face_composition_catalogs(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_plot::{KindSignature, StartupParameterSignature};
    for contract in [
        face_icon_contract(),
        face_frame_contract(),
        face_badge_contract(),
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
                        ConfigurationValue::Text(value) => {
                            if matches!(field.rule, KindConfigurationRule::DataLabel { .. }) {
                                None
                            } else {
                                Some(alloc::format!("{value:?}"))
                            }
                        }
                        _ => unreachable!(),
                    },
                })
                .collect(),
        })?;
        profile
            .insert_kind(contract.into_semantic_contract(FACE_COMPOSITION_CONTRACT_REVISION))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TEXT_FACE_KIND;

    #[test]
    fn family_reuses_text_and_has_no_renderer_mechanics() {
        let contracts = [
            face_icon_contract(),
            face_frame_contract(),
            face_badge_contract(),
        ];
        assert!(contracts.iter().all(|contract| {
            contract
                .inputs
                .iter()
                .chain(&contract.outputs)
                .all(|port| port.value_kind.as_str() == FACE_COMPOSITION_KIND)
        }));
        assert!(!contracts
            .iter()
            .any(|contract| contract.kind_id.as_str() == TEXT_FACE_KIND));
        assert_eq!(
            super::super::text_face_contract().kind_id.as_str(),
            TEXT_FACE_KIND
        );
        assert!(contracts.iter().all(|contract| {
            contract
                .configuration
                .iter()
                .any(|field| field.key == "label")
                && contract
                    .configuration
                    .iter()
                    .all(|field| field.key != "accessibility-name")
        }));
    }
}
