use crate::prelude::*;
use crate::syntax::{
    Argument, BackStatement, CordStage, Expression, ExpressionSyntax, Invocation, PlotSyntax,
    SpannedText,
};
use alloc::collections::BTreeMap;
use alloc::format;

pub(super) fn plot(
    syntax: &mut PlotSyntax,
    type_substitutions: &BTreeMap<String, String>,
    behavior_substitutions: &BTreeMap<String, String>,
) {
    for parameter in &mut syntax.front.startup_parameters {
        value_type(&mut parameter.value_type, type_substitutions);
        if let Some(default) = &mut parameter.default {
            expression(default, type_substitutions, behavior_substitutions);
        }
    }
    for port in &mut syntax.front.runtime_ports {
        value_type(&mut port.value_type, type_substitutions);
    }
    for statement in &mut syntax.back {
        match statement {
            BackStatement::NamedGear(gear) => invocation(
                &mut gear.invocation,
                type_substitutions,
                behavior_substitutions,
            ),
            BackStatement::Cord(cord) => {
                stages(&mut cord.stages, type_substitutions, behavior_substitutions)
            }
            BackStatement::MatchedRoute(route) => {
                for arm in &mut route.arms {
                    stages(&mut arm.stages, type_substitutions, behavior_substitutions);
                }
            }
            BackStatement::LocalValue(value) => {
                expression(&mut value.value, type_substitutions, behavior_substitutions)
            }
            BackStatement::Pool(_) => {}
        }
    }
    for local in &mut syntax.local_plots {
        plot(local, type_substitutions, behavior_substitutions);
    }
}

pub(super) fn stages(
    stages: &mut [CordStage],
    type_substitutions: &BTreeMap<String, String>,
    behavior_substitutions: &BTreeMap<String, String>,
) {
    for stage in stages {
        match stage {
            CordStage::InlineGear(value)
            | CordStage::RelationalGear {
                invocation: value, ..
            } => invocation(value, type_substitutions, behavior_substitutions),
            CordStage::StructuredSelector(selector) => match selector {
                crate::StructuredSelectorSyntax::Field {
                    value_type: value, ..
                }
                | crate::StructuredSelectorSyntax::Index {
                    value_type: value, ..
                }
                | crate::StructuredSelectorSyntax::Variant {
                    value_type: value, ..
                } => value_type(value, type_substitutions),
            },
            CordStage::When(value)
            | CordStage::Literal(value)
            | CordStage::PureExpression(value) => {
                expression(value, type_substitutions, behavior_substitutions)
            }
            _ => {}
        }
    }
}

pub(super) fn invocation(
    invocation: &mut Invocation,
    type_substitutions: &BTreeMap<String, String>,
    behavior_substitutions: &BTreeMap<String, String>,
) {
    if let Some(selected) = behavior_substitutions.get(&invocation.kind.text) {
        invocation.kind.text.clone_from(selected);
    }
    for argument in &mut invocation.arguments {
        let value = match argument {
            Argument::Positional(value) | Argument::Named { value, .. } => value,
        };
        expression(value, type_substitutions, behavior_substitutions);
    }
}

pub(super) fn expression(
    expression: &mut Expression,
    type_substitutions: &BTreeMap<String, String>,
    behavior_substitutions: &BTreeMap<String, String>,
) {
    expression_syntax(
        &mut expression.syntax,
        type_substitutions,
        behavior_substitutions,
    );
    if let ExpressionSyntax::Atomic(atom) = &expression.syntax {
        expression.text.clone_from(&atom.text);
    }
}

fn expression_syntax(
    expression: &mut ExpressionSyntax,
    type_substitutions: &BTreeMap<String, String>,
    behavior_substitutions: &BTreeMap<String, String>,
) {
    match expression {
        ExpressionSyntax::Atomic(value) => {
            value_type(value, type_substitutions);
            if let Some(selected) = behavior_substitutions.get(&value.text) {
                value.text.clone_from(selected);
            }
        }
        ExpressionSyntax::Projection { value, .. }
        | ExpressionSyntax::Unary { operand: value, .. } => {
            expression_syntax(value, type_substitutions, behavior_substitutions)
        }
        ExpressionSyntax::Binary { left, right, .. } => {
            expression_syntax(left, type_substitutions, behavior_substitutions);
            expression_syntax(right, type_substitutions, behavior_substitutions);
        }
        ExpressionSyntax::Conditional {
            condition,
            when_true,
            when_false,
            ..
        } => {
            expression_syntax(condition, type_substitutions, behavior_substitutions);
            expression_syntax(when_true, type_substitutions, behavior_substitutions);
            expression_syntax(when_false, type_substitutions, behavior_substitutions);
        }
        ExpressionSyntax::Tuple { values, .. } | ExpressionSyntax::Collection { values, .. } => {
            for value in values {
                expression_syntax(value, type_substitutions, behavior_substitutions);
            }
        }
        ExpressionSyntax::Record { fields, .. } => {
            for field in fields {
                expression_syntax(&mut field.value, type_substitutions, behavior_substitutions);
            }
        }
        ExpressionSyntax::Variant { tag, payload, span } => {
            expression_syntax(payload, type_substitutions, behavior_substitutions);
            if let Some(selected) = behavior_substitutions.get(&tag.text) {
                *expression = ExpressionSyntax::SemanticCall {
                    kind: SpannedText {
                        text: selected.clone(),
                        span: tag.span,
                    },
                    arguments: vec![*payload.clone()],
                    span: *span,
                };
            }
        }
        ExpressionSyntax::SemanticCall {
            kind, arguments, ..
        } => {
            if let Some(selected) = behavior_substitutions.get(&kind.text) {
                kind.text.clone_from(selected);
            }
            for argument in arguments {
                expression_syntax(argument, type_substitutions, behavior_substitutions);
            }
        }
        ExpressionSyntax::Input(_) => {}
    }
}

pub(super) fn value_type(value: &mut SpannedText, substitutions: &BTreeMap<String, String>) {
    if let Some(concrete) = substitutions.get(&value.text) {
        value.text.clone_from(concrete);
    } else if let Some(inner) = value.text.strip_prefix('&') {
        if let Some(concrete) = substitutions.get(inner) {
            value.text = format!("&{concrete}");
        }
    }
}
