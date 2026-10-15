//! Inspectable lowering of named expression-body inputs into ordinary cords.
use super::*;

pub(super) fn expression_body_expansion(plot: &PlotSyntax) -> Option<PendingSourceSugarExpansion> {
    let body = plot.expression_body.as_ref()?;
    let input = plot
        .front
        .runtime_ports
        .iter()
        .find(|port| port.direction == crate::RuntimePortDirection::Input)?;
    let output = plot
        .front
        .runtime_ports
        .iter()
        .find(|port| port.direction == crate::RuntimePortDirection::Output)?;
    let mut expression = body.expression.clone();
    crate::surface_parser::expression_body::bind_input(&mut expression.syntax, &input.name.text);
    let mut replacements = Vec::new();
    input_spans(&expression.syntax, &mut replacements);
    replacements.sort_by_key(|span| core::cmp::Reverse(span.start));
    let mut anonymous = expression.text;
    for span in replacements {
        anonymous.replace_range(
            span.start - expression.span.start..span.end - expression.span.start,
            ".",
        );
    }
    Some(PendingSourceSugarExpansion {
        plot: plot.name.text.clone(),
        authored: alloc::format!("= {}", body.expression.text),
        source_span: body.span,
        ordinary_kind: "conduitese/pure-expression-operation@1".into(),
        input_ports: alloc::vec![input.name.text.clone()],
        output_ports: alloc::vec![output.name.text.clone()],
        operand_bindings: alloc::vec![SourceSugarOperandBinding {
            source: input.name.text.clone(),
            input_port: "value".into(),
        }],
        canonical_replacement: Some(alloc::format!(
            "{{\n    {} >> {} >> {}\n}}",
            input.name.text,
            anonymous,
            output.name.text
        )),
    })
}

fn input_spans(expression: &crate::ExpressionSyntax, spans: &mut Vec<crate::Span>) {
    use crate::ExpressionSyntax as E;
    match expression {
        E::Input(span) => spans.push(*span),
        E::Projection { value, .. }
        | E::Unary { operand: value, .. }
        | E::Variant { payload: value, .. } => input_spans(value, spans),
        E::Binary { left, right, .. } => {
            input_spans(left, spans);
            input_spans(right, spans);
        }
        E::Conditional {
            condition,
            when_true,
            when_false,
            ..
        } => {
            input_spans(condition, spans);
            input_spans(when_true, spans);
            input_spans(when_false, spans);
        }
        E::Tuple { values, .. }
        | E::Collection { values, .. }
        | E::SemanticCall {
            arguments: values, ..
        } => {
            for value in values {
                input_spans(value, spans);
            }
        }
        E::Record { fields, .. } => {
            for field in fields {
                input_spans(&field.value, spans);
            }
        }
        _ => {}
    }
}
