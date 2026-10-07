//! Portable exact contract for one checked Conduitese pure expression.

use alloc::vec;
use conduit_core::{
    CapabilityLimits, FrontStartupParameter, Kind, PortTemporal, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

pub fn pure_expression_contract(
    program: &conduit_plot::PortableExpressionProgram,
    temporal: PortTemporal,
) -> Result<Kind, conduit_core::StructuredInfoRefusal> {
    let definition = conduit_plot::portable_expression_definition(program, temporal)?;
    Ok(Kind {
        startup_parameters: vec![FrontStartupParameter {
            name: "program".into(),
            value_type: conduit_core::kind_id("value/text"),
            has_default: false,
        }],
        shorthand: Some((
            conduit_core::port_id("input"),
            conduit_core::port_id("output"),
        )),
        kind_id: definition.kind_id,
        kind_contract_revision: definition.kind_contract_revision,
        inputs: definition.inputs,
        outputs: definition.outputs,
        configuration: definition.configuration,
        semantic_laws: conduit_plot::pure_expression_semantic_laws_for_program(program)?,
        limits: CapabilityLimits {
            // Bounded unrolled protocol walks may repeat one checked expression
            // sixteen times. Each instance still has separately admitted storage.
            max_active_instances: 16,
            max_queue_items: 4,
            max_queue_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 4) as u32,
        },
    })
}

pub fn pure_filter_contract(
    program: &conduit_plot::PortableExpressionProgram,
    temporal: PortTemporal,
) -> Result<Kind, conduit_core::StructuredInfoRefusal> {
    Ok(contract(conduit_plot::portable_filter_definition(
        program, temporal,
    )?))
}

fn contract(definition: conduit_plot::KindProjection) -> Kind {
    Kind {
        startup_parameters: vec![FrontStartupParameter {
            name: "program".into(),
            value_type: conduit_core::kind_id("value/text"),
            has_default: false,
        }],
        shorthand: Some((
            conduit_core::port_id("input"),
            conduit_core::port_id("output"),
        )),
        kind_id: definition.kind_id,
        kind_contract_revision: definition.kind_contract_revision,
        inputs: definition.inputs,
        outputs: definition.outputs,
        configuration: definition.configuration,
        semantic_laws: conduit_plot::pure_expression_semantic_laws(),
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 4,
            max_queue_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 4) as u32,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::collections::{BTreeMap, BTreeSet};
    use conduit_plot::{
        check_expression, parse_syntax_document, BackStatement, CheckedExpressionType, CordStage,
        ExpressionTypeContext,
    };

    #[test]
    fn semantic_contract_is_the_exact_plot_projection_and_is_derived_pure() {
        let source =
            "plot checked (\n input: U8 >> output: U8\n) {\n input >> (. + 1) >> output\n}\n";
        let syntax = parse_syntax_document(source);
        let BackStatement::Cord(cord) = &syntax.plots[0].back[0] else {
            panic!("cord")
        };
        let CordStage::PureExpression(expression) = &cord.stages[1] else {
            panic!("expression")
        };
        let input = CheckedExpressionType::semantic("value/u8");
        let immutable_values = BTreeMap::new();
        let structured_types = BTreeMap::new();
        let literal_types = BTreeMap::new();
        let semantic_kinds = BTreeMap::new();
        let numeric = BTreeSet::new();
        let checked = check_expression(
            &expression.syntax,
            &ExpressionTypeContext {
                input: &input,
                immutable_values: &immutable_values,
                structured_types: &structured_types,
                literal_types: &literal_types,
                numeric_types: &numeric,
                semantic_kinds: &semantic_kinds,
            },
        )
        .unwrap();
        let program = conduit_plot::PortableExpressionProgram::from_checked(&checked).unwrap();
        let definition = conduit_plot::portable_expression_definition(
            &program,
            conduit_core::PortTemporal::Value,
        )
        .unwrap();
        let contract =
            pure_expression_contract(&program, conduit_core::PortTemporal::Value).unwrap();
        assert_eq!(contract.kind_id, definition.kind_id);
        assert_eq!(contract.inputs, definition.inputs);
        assert_eq!(contract.outputs, definition.outputs);
        assert!(conduit_core::pure_expression_facts(&contract).is_ok());
    }
}
