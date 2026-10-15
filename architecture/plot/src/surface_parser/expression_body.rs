//! Expression bodies bind their one declared runtime input without changing source evidence.
use super::*;
use crate::ExpressionSyntax as E;

pub(super) fn expression_body_cord(
    front: &PlotFront,
    mut expression: Expression,
    span: Span,
) -> Result<BackStatement, (PlotError, Span)> {
    let inputs = front
        .runtime_ports
        .iter()
        .filter(|port| port.direction == RuntimePortDirection::Input)
        .collect::<Vec<_>>();
    let outputs = front
        .runtime_ports
        .iter()
        .filter(|port| port.direction == RuntimePortDirection::Output)
        .collect::<Vec<_>>();
    if inputs.len() != 1 || outputs.len() != 1 || inputs[0].temporal != outputs[0].temporal {
        return Err((
            PlotError::InvalidSyntax(
                "expression-bodied plot requires exactly one runtime input and one runtime output with the same temporal contract; write an ordinary explicit Plot back"
                    .into(),
            ),
            span,
        ));
    }
    bind_input(&mut expression.syntax, &inputs[0].name.text);
    Ok(BackStatement::Cord(Cord {
        stages: vec![
            CordStage::Reference(inputs[0].name.clone()),
            CordStage::PureExpression(expression),
            CordStage::Reference(outputs[0].name.clone()),
        ],
        span,
    }))
}

pub(crate) fn bind_input(expression: &mut E, name: &str) {
    match expression {
        E::Atomic(value) if value.text == name => *expression = E::Input(value.span),
        E::Projection { value, .. }
        | E::Unary { operand: value, .. }
        | E::Variant { payload: value, .. } => bind_input(value, name),
        E::Binary { left, right, .. } => {
            bind_input(left, name);
            bind_input(right, name);
        }
        E::Conditional {
            condition,
            when_true,
            when_false,
            ..
        } => {
            bind_input(condition, name);
            bind_input(when_true, name);
            bind_input(when_false, name);
        }
        E::Tuple { values, .. }
        | E::Collection { values, .. }
        | E::SemanticCall {
            arguments: values, ..
        } => {
            for value in values {
                bind_input(value, name);
            }
        }
        E::Record { fields, .. } => {
            for field in fields {
                bind_input(&mut field.value, name);
            }
        }
        _ => {}
    }
}
