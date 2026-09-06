//! Portable button-to-indicator contracts and deterministic semantic mapping.
//!
//! Device grouping and Host realization remain outside these values. Ordered
//! button transitions map to a current desired indicator state: pressed is on,
//! released is off.

use super::{input_button_transition_type, StandardKindContract, TerminalBehavior};
mod prepared;
#[cfg(feature = "form-catalog")]
use alloc::string::String;
use alloc::{string::ToString, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, ConfigurationValue, InfoBool, PortDescriptor,
    PortDirection, PortTemporal, StructuredFieldValue, StructuredInfoRefusal, StructuredInfoType,
    StructuredInfoValue, StructuredInfoValueShape, BOOL_INFO_ID,
};
pub use prepared::PreparedButtonIndicatorMapper;

pub const BUTTON_SOURCE_KIND: &str = "input/button";
pub const BUTTON_SOURCE_REVISION: &str = "conduit.input/button@2";
pub const BUTTON_INDICATOR_STATE_KIND: &str = "input/button-indicator-state";
pub const BUTTON_INDICATOR_STATE_REVISION: &str = "conduit.input/button-indicator-state@1";
pub const INDICATOR_STATE_PRESENTATION_KIND: &str = "presentation/indicator-state";
pub const INDICATOR_STATE_PRESENTATION_REVISION: &str = "conduit.presentation/indicator-state@1";
pub const BUTTON_TRANSITION_MAXIMUM_BYTES: u32 =
    conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32;
pub const BUTTON_TRANSITION_MAXIMUM_VALUES: u16 = 8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ButtonIndicatorRefusal {
    Malformed(StructuredInfoRefusal),
    Selection(conduit_core::StructuredSelectorRefusal),
    WrongType,
    MissingPhase,
    UnknownPhase,
}

pub fn button_source_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(BUTTON_SOURCE_KIND),
        plain_name: "Button transitions".to_string(),
        summary: "Observe a bounded ordered stream of semantic press and release transitions."
            .to_string(),
        inputs: Vec::new(),
        outputs: vec![button_port("transition", PortDirection::Output)],
        configuration: vec![crate::StandardConfigurationField {
            key: "maximum-transitions".into(),
            default_value: ConfigurationValue::U64(2),
            rule: crate::StandardConfigurationRule::U64Range {
                minimum: 1,
                maximum: u64::from(BUTTON_TRANSITION_MAXIMUM_VALUES),
            },
        }],
        limits: button_limits(),
        terminal_behavior: TerminalBehavior::HostInputEndsOrFailsSource,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "button: input/button".to_string(),
    }
}

pub fn button_indicator_state_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(BUTTON_INDICATOR_STATE_KIND),
        plain_name: "Button to indicator state".to_string(),
        summary: "Map every ordered press or release to the current desired indicator state."
            .to_string(),
        inputs: vec![button_port("transition", PortDirection::Input)],
        outputs: vec![PortDescriptor {
            port_id: port_id("state"),
            value_kind: kind_id(BOOL_INFO_ID),
            direction: PortDirection::Output,
            temporal: PortTemporal::Current,
        }],
        configuration: Vec::new(),
        limits: button_limits(),
        terminal_behavior: TerminalBehavior::MirrorsInputTerminal,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "state: input/button-indicator-state".to_string(),
    }
}

pub fn indicator_state_presentation_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(INDICATOR_STATE_PRESENTATION_KIND),
        plain_name: "Indicator state".to_string(),
        summary: "Manifest one current semantic indicator state through admitted Host machinery."
            .to_string(),
        inputs: vec![PortDescriptor {
            port_id: port_id("state"),
            value_kind: kind_id(BOOL_INFO_ID),
            direction: PortDirection::Input,
            temporal: PortTemporal::Current,
        }],
        outputs: Vec::new(),
        configuration: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 1,
            max_queue_bytes: 1,
        },
        terminal_behavior: TerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true,
        browser_manifestation_honest: true,
        pico_manifestation_honest: false,
        example: "indicator: presentation/indicator-state".to_string(),
    }
}

pub fn map_button_transition_to_indicator(
    encoded: &[u8],
) -> Result<InfoBool, ButtonIndicatorRefusal> {
    let value = StructuredInfoValue::from_canonical_bytes(encoded)
        .map_err(ButtonIndicatorRefusal::Malformed)?;
    if value.value_type() != &input_button_transition_type() {
        return Err(ButtonIndicatorRefusal::WrongType);
    }
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        return Err(ButtonIndicatorRefusal::WrongType);
    };
    let phase = fields
        .iter()
        .find(|field| field.name() == "phase")
        .ok_or(ButtonIndicatorRefusal::MissingPhase)?;
    let StructuredInfoValueShape::Variant { tag, .. } = phase.value().shape() else {
        return Err(ButtonIndicatorRefusal::WrongType);
    };
    match tag {
        "pressed" => Ok(InfoBool::TRUE),
        "released" => Ok(InfoBool::FALSE),
        _ => Err(ButtonIndicatorRefusal::UnknownPhase),
    }
}

pub fn button_transition_value(
    button_identity: &str,
    pressed: bool,
    sequence: u64,
) -> Result<StructuredInfoValue, StructuredInfoRefusal> {
    let leaf = |kind: &str, bytes: Vec<u8>| {
        StructuredInfoValue::leaf(StructuredInfoType::leaf(kind_id(kind))?, bytes)
    };
    let phase = StructuredInfoValue::variant(
        super::input_button_phase_type(),
        if pressed { "pressed" } else { "released" },
        leaf("value/unit@1", Vec::new())?,
    )?;
    StructuredInfoValue::record(
        input_button_transition_type(),
        vec![
            StructuredFieldValue::new(
                "button_identity",
                leaf("value/text@1", button_identity.as_bytes().to_vec())?,
            )?,
            StructuredFieldValue::new("phase", phase)?,
            StructuredFieldValue::new(
                "sequence",
                leaf("value/count@1", sequence.to_string().into_bytes())?,
            )?,
        ],
    )
}

fn button_port(name: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: input_button_transition_type()
            .profile()
            .expect("reviewed button transition profile")
            .value_kind()
            .clone(),
        direction,
        temporal: PortTemporal::Flow { closes: true },
    }
}

fn button_limits() -> CapabilityLimits {
    CapabilityLimits {
        max_active_instances: 8,
        max_queue_items: BUTTON_TRANSITION_MAXIMUM_VALUES,
        max_queue_bytes: BUTTON_TRANSITION_MAXIMUM_BYTES
            * u32::from(BUTTON_TRANSITION_MAXIMUM_VALUES),
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_button_indicator_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    use conduit_core::KindContractRevision;
    use conduit_form::{KindDefinition, KindSignature};
    for (contract, revision) in [
        (button_source_contract(), BUTTON_SOURCE_REVISION),
        (
            button_indicator_state_contract(),
            BUTTON_INDICATOR_STATE_REVISION,
        ),
        (
            indicator_state_presentation_contract(),
            INDICATOR_STATE_PRESENTATION_REVISION,
        ),
    ] {
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().to_string(),
            startup_parameters: contract
                .configuration
                .iter()
                .map(|field| conduit_form::StartupParameterSignature {
                    name: field.key.clone(),
                    value_type: "Count".into(),
                    default: Some("2".into()),
                })
                .collect(),
        })?;
        profile
            .insert(KindDefinition {
                kind_id: contract.kind_id,
                kind_contract_revision: KindContractRevision::from(revision),
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration: contract
                    .configuration
                    .into_iter()
                    .map(|field| conduit_form::ConfigurationField {
                        key: field.key,
                        default_value: field.default_value,
                        validation: conduit_form::ConfigurationRule::U64Range {
                            minimum: 1,
                            maximum: u64::from(BUTTON_TRANSITION_MAXIMUM_VALUES),
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
    use conduit_core::StructuredInfoType;

    fn leaf(kind: &str, bytes: &[u8]) -> StructuredInfoValue {
        StructuredInfoValue::leaf(
            StructuredInfoType::leaf(kind_id(kind)).unwrap(),
            bytes.to_vec(),
        )
        .unwrap()
    }

    fn transition(phase: &str, sequence: u8) -> Vec<u8> {
        button_transition_value("button/primary", phase == "pressed", u64::from(sequence))
            .unwrap()
            .canonical_bytes()
            .unwrap()
    }

    #[test]
    fn ordered_phases_map_to_current_indicator_state() {
        assert_eq!(
            map_button_transition_to_indicator(&transition("pressed", 1)),
            Ok(InfoBool::TRUE)
        );
        assert_eq!(
            map_button_transition_to_indicator(&transition("released", 2)),
            Ok(InfoBool::FALSE)
        );
    }

    #[test]
    fn contracts_keep_ordered_transition_and_current_state_distinct() {
        let mapping = button_indicator_state_contract();
        assert_eq!(
            mapping.inputs[0].temporal,
            PortTemporal::Flow { closes: true }
        );
        assert_eq!(mapping.outputs[0].temporal, PortTemporal::Current);
        assert_eq!(
            indicator_state_presentation_contract()
                .limits
                .max_queue_items,
            1
        );
    }

    #[test]
    fn unrelated_structured_values_refuse_instead_of_becoming_indicator_state() {
        let unrelated = leaf("value/text@1", b"pressed").canonical_bytes().unwrap();
        assert_eq!(
            map_button_transition_to_indicator(&unrelated),
            Err(ButtonIndicatorRefusal::WrongType)
        );
    }
}
