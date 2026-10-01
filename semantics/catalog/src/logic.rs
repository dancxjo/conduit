use super::{
    KindConfigurationField, KindConfigurationRule, KindTerminalBehavior, StandardKindContract,
};
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    kind_id, port_id, CapabilityLimits, ConfigurationValue, PortDescriptor, PortDirection,
    PortTemporal, BOOL_INFO_ID, SCALAR_INFO_ID,
};
pub use conduit_data::ScalarComparison;

pub const LOGIC_COMPARE_KIND: &str = "logic/compare";
pub const LOGIC_NOT_KIND: &str = "logic/not";
pub const LOGIC_SELECT_KIND: &str = "logic/select";

pub const LOGIC_COMPARE_SCALAR_CONTRACT_REVISION: &str = "conduit.std/logic-compare-scalar@1";
pub const LOGIC_NOT_CONTRACT_REVISION: &str = "conduit.std/logic-not@1";
pub const LOGIC_SELECT_SCALAR_CONTRACT_REVISION: &str = "conduit.std/logic-select-scalar@1";

pub fn parse_scalar_comparison(value: &str) -> Option<ScalarComparison> {
    match value {
        "lt" => Some(ScalarComparison::Less),
        "le" => Some(ScalarComparison::LessOrEqual),
        "eq" => Some(ScalarComparison::Equal),
        "ne" => Some(ScalarComparison::NotEqual),
        "ge" => Some(ScalarComparison::GreaterOrEqual),
        "gt" => Some(ScalarComparison::Greater),
        _ => None,
    }
}

pub const fn select_scalar(
    selector: bool,
    when_false: conduit_core::Scalar,
    when_true: conduit_core::Scalar,
) -> conduit_core::Scalar {
    if selector {
        when_true
    } else {
        when_false
    }
}

pub const COMPARE_LEFT_PORT: &str = "left";
pub const COMPARE_RIGHT_PORT: &str = "right";
pub const LOGIC_INPUT_PORT: &str = "in";
pub const LOGIC_OUTPUT_PORT: &str = "out";
pub const SELECT_SELECTOR_PORT: &str = "selector";
pub const SELECT_FALSE_PORT: &str = "when-false";
pub const SELECT_TRUE_PORT: &str = "when-true";
pub const COMPARE_OPERATOR_KEY: &str = "operator";
pub const COMPARISON_OPERATORS: [&str; 6] = ["lt", "le", "eq", "ne", "ge", "gt"];

pub fn logic_compare_scalar_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(LOGIC_COMPARE_KIND),
        plain_name: "Compare scalars".to_string(),
        summary: "Compare two exact scalar values with one finite configured operator.".to_string(),
        inputs: vec![
            value_port(COMPARE_LEFT_PORT, SCALAR_INFO_ID, PortDirection::Input),
            value_port(COMPARE_RIGHT_PORT, SCALAR_INFO_ID, PortDirection::Input),
        ],
        outputs: vec![value_port(
            LOGIC_OUTPUT_PORT,
            BOOL_INFO_ID,
            PortDirection::Output,
        )],
        configuration: vec![KindConfigurationField {
            key: COMPARE_OPERATOR_KEY.to_string(),
            default_value: ConfigurationValue::Text("eq".to_string()),
            rule: KindConfigurationRule::TextOneOf {
                values: comparison_operator_values(),
            },
        }],
        limits: limits(),
        terminal_behavior:
            KindTerminalBehavior::EmitsOneDecisionOrCompletesWhenDecisionBecomesImpossible,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "compare: logic/compare(operator = \"lt\")".to_string(),
    }
}

pub fn logic_not_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(LOGIC_NOT_KIND),
        plain_name: "Boolean not".to_string(),
        summary: "Invert one exact Boolean value without truthiness coercion.".to_string(),
        inputs: vec![value_port(
            LOGIC_INPUT_PORT,
            BOOL_INFO_ID,
            PortDirection::Input,
        )],
        outputs: vec![value_port(
            LOGIC_OUTPUT_PORT,
            BOOL_INFO_ID,
            PortDirection::Output,
        )],
        configuration: Default::default(),
        limits: limits(),
        terminal_behavior:
            KindTerminalBehavior::EmitsOneDecisionOrCompletesWhenDecisionBecomesImpossible,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "invert: logic/not".to_string(),
    }
}

pub fn logic_select_scalar_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(LOGIC_SELECT_KIND),
        plain_name: "Select scalar".to_string(),
        summary: "Select one of two exact scalar values using one exact Boolean.".to_string(),
        inputs: vec![
            value_port(SELECT_SELECTOR_PORT, BOOL_INFO_ID, PortDirection::Input),
            value_port(SELECT_FALSE_PORT, SCALAR_INFO_ID, PortDirection::Input),
            value_port(SELECT_TRUE_PORT, SCALAR_INFO_ID, PortDirection::Input),
        ],
        outputs: vec![value_port(
            LOGIC_OUTPUT_PORT,
            SCALAR_INFO_ID,
            PortDirection::Output,
        )],
        configuration: Default::default(),
        limits: limits(),
        terminal_behavior:
            KindTerminalBehavior::EmitsOneDecisionOrCompletesWhenDecisionBecomesImpossible,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "choice: logic/select".to_string(),
    }
}

fn value_port(name: &str, info: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(info),
        direction,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    }
}

fn limits() -> CapabilityLimits {
    CapabilityLimits {
        max_active_instances: 16,
        max_queue_items: 1,
        max_queue_bytes: 8,
    }
}

fn comparison_operator_values() -> Vec<String> {
    COMPARISON_OPERATORS
        .iter()
        .map(|operator| (*operator).to_string())
        .collect()
}

#[cfg(feature = "form-catalog")]
pub fn install_logic_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    use conduit_form::KindSignature;
    for (contract, revision) in [
        (
            logic_compare_scalar_contract(),
            LOGIC_COMPARE_SCALAR_CONTRACT_REVISION,
        ),
        (logic_not_contract(), LOGIC_NOT_CONTRACT_REVISION),
        (
            logic_select_scalar_contract(),
            LOGIC_SELECT_SCALAR_CONTRACT_REVISION,
        ),
    ] {
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().to_string(),
            startup_parameters: contract
                .configuration
                .iter()
                .map(|field| conduit_form::StartupParameterSignature {
                    name: field.key.clone(),
                    value_type: "Text".to_string(),
                    default: Some(match &field.default_value {
                        ConfigurationValue::Text(value) => value.clone(),
                        _ => unreachable!("logic configuration is finite text"),
                    }),
                })
                .collect(),
        })?;
        profile
            .insert_kind(contract.into_semantic_contract(revision))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logic_contracts_are_exact_one_shot_and_finite() {
        let compare = logic_compare_scalar_contract();
        assert_eq!(compare.inputs[0].value_kind.as_str(), SCALAR_INFO_ID);
        assert_eq!(compare.outputs[0].value_kind.as_str(), BOOL_INFO_ID);
        assert!(compare
            .inputs
            .iter()
            .chain(compare.outputs.iter())
            .all(|port| port.temporal == PortTemporal::Value));
        assert!(matches!(
            &compare.configuration[0].rule,
            KindConfigurationRule::TextOneOf { values }
                if values == &comparison_operator_values()
        ));

        let not = logic_not_contract();
        assert_eq!(not.inputs[0].value_kind.as_str(), BOOL_INFO_ID);
        assert_eq!(not.outputs[0].value_kind.as_str(), BOOL_INFO_ID);

        let select = logic_select_scalar_contract();
        assert_eq!(select.inputs[0].value_kind.as_str(), BOOL_INFO_ID);
        assert!(select.inputs[1..]
            .iter()
            .chain(select.outputs.iter())
            .all(|port| port.value_kind.as_str() == SCALAR_INFO_ID));
    }
}
