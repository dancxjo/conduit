use crate::prelude::*;
use crate::syntax::{
    Argument, BackStatement, CordStage, Expression, ExpressionSyntax, FormSyntax, Invocation,
    MatchedRoutePattern,
};
use crate::{Span, SyntaxCheckDiagnostic};
use alloc::collections::{BTreeMap, BTreeSet};

const LOCAL_FORM_PREFIX: &str = "$local/";

#[derive(Clone)]
struct LocalBinding {
    identity: String,
    captured_types: Vec<crate::TypeParameter>,
    captured_kinds: Vec<crate::KindParameter>,
}

/// Lowers lexical Forms to ordinary source Forms before the normal checker.
/// The scoped identity contains `$`, which authored operation names cannot
/// contain, so a private helper cannot be addressed from another source scope.
pub(super) fn lower(forms: &[FormSyntax]) -> Result<Vec<FormSyntax>, SyntaxCheckDiagnostic> {
    let mut lowered = Vec::new();
    for form in forms {
        lower_form(
            form.clone(),
            &[],
            &BTreeMap::new(),
            &BTreeSet::new(),
            0,
            false,
            &mut lowered,
        )?;
    }
    Ok(lowered)
}

pub(super) fn is_local_identity(name: &str) -> bool {
    name.starts_with(LOCAL_FORM_PREFIX)
}

#[allow(clippy::too_many_arguments)]
fn lower_form(
    mut form: FormSyntax,
    parent_path: &[String],
    inherited: &BTreeMap<String, LocalBinding>,
    inherited_values: &BTreeSet<String>,
    depth: usize,
    local: bool,
    lowered: &mut Vec<FormSyntax>,
) -> Result<(), SyntaxCheckDiagnostic> {
    if depth > crate::MAXIMUM_FORM_NESTING_DEPTH {
        return Err(diagnostic(
            form.span,
            format!(
                "local Form nesting exceeds the {}-level bound",
                crate::MAXIMUM_FORM_NESTING_DEPTH
            ),
        ));
    }

    let authored_name = form.name.text.clone();
    let mut path = parent_path.to_vec();
    path.push(authored_name.clone());
    if local {
        form.name.text = format!("{LOCAL_FORM_PREFIX}{}", path.join("/"));
    }

    let mut local_forms = core::mem::take(&mut form.local_forms);
    let fore_bindings = form
        .front
        .startup_parameters
        .iter()
        .map(|parameter| parameter.name.text.clone())
        .chain(
            form.front
                .runtime_ports
                .iter()
                .map(|port| port.name.text.clone()),
        )
        .collect::<BTreeSet<_>>();
    let mut visible_values = inherited_values.clone();
    visible_values.extend(form_value_bindings(&form));
    let mut visible = inherited.clone();
    let mut declared = BTreeSet::new();
    for child in &mut local_forms {
        if !declared.insert(child.name.text.clone())
            || visible.contains_key(&child.name.text)
            || fore_bindings.contains(&child.name.text)
        {
            return Err(diagnostic(
                child.name.span,
                format!(
                    "local Form '{}' duplicates or shadows a visible binding",
                    child.name.text
                ),
            ));
        }
        let mut child_path = path.clone();
        child_path.push(child.name.text.clone());
        for parameter in &form.front.type_parameters {
            if child
                .front
                .type_parameters
                .iter()
                .any(|own| own.name.text == parameter.name.text)
            {
                return Err(diagnostic(
                    child.name.span,
                    format!(
                        "local Form '{}' cannot shadow outer compile-time type parameter '{}'",
                        child.name.text, parameter.name.text
                    ),
                ));
            }
        }
        for parameter in &form.front.kind_parameters {
            if child
                .front
                .kind_parameters
                .iter()
                .any(|own| own.name.text == parameter.name.text)
            {
                return Err(diagnostic(
                    child.name.span,
                    format!(
                        "local Form '{}' cannot shadow outer compile-time behavior parameter '{}'",
                        child.name.text, parameter.name.text
                    ),
                ));
            }
        }
        let captured_types = form.front.type_parameters.clone();
        let captured_kinds = form.front.kind_parameters.clone();
        child
            .front
            .type_parameters
            .splice(0..0, captured_types.iter().cloned());
        child
            .front
            .kind_parameters
            .splice(0..0, captured_kinds.iter().cloned());
        visible.insert(
            child.name.text.clone(),
            LocalBinding {
                identity: format!("{LOCAL_FORM_PREFIX}{}", child_path.join("/")),
                captured_types,
                captured_kinds,
            },
        );
    }

    rewrite_form(&mut form, &visible);
    lowered.push(form);
    for child in local_forms {
        reject_implicit_capture(&child, &visible_values)?;
        lower_form(
            child,
            &path,
            &visible,
            &visible_values,
            depth + 1,
            true,
            lowered,
        )?;
    }
    Ok(())
}

fn form_value_bindings(form: &FormSyntax) -> BTreeSet<String> {
    form.front
        .startup_parameters
        .iter()
        .map(|parameter| parameter.name.text.clone())
        .chain(
            form.front
                .runtime_ports
                .iter()
                .map(|port| port.name.text.clone()),
        )
        .chain(form.back.iter().filter_map(|statement| match statement {
            BackStatement::NamedGear(gear) => Some(gear.name.text.clone()),
            BackStatement::Pool(pool) => Some(pool.name.text.clone()),
            BackStatement::LocalValue(value) => Some(value.name.text.clone()),
            BackStatement::Cord(_) | BackStatement::MatchedRoute(_) => None,
        }))
        .collect()
}

fn reject_implicit_capture(
    form: &FormSyntax,
    outer: &BTreeSet<String>,
) -> Result<(), SyntaxCheckDiagnostic> {
    let own = form_value_bindings(form);
    let captured = outer.difference(&own).collect::<BTreeSet<_>>();
    let mut use_name = |name: &str, span: Span| {
        if captured.iter().any(|candidate| candidate.as_str() == name) {
            Err(diagnostic(
                span,
                format!(
                    "local Form '{}' cannot implicitly capture outer binding '{name}'; pass it through the local Form Fore or startup parameters",
                    form.name.text
                ),
            ))
        } else {
            Ok(())
        }
    };
    for parameter in &form.front.startup_parameters {
        if let Some(default) = &parameter.default {
            visit_expression(&default.syntax, &mut use_name)?;
        }
    }
    for statement in &form.back {
        match statement {
            BackStatement::NamedGear(gear) => {
                use_name(&gear.invocation.kind.text, gear.invocation.kind.span)?;
                visit_invocation(&gear.invocation, &mut use_name)?;
                if let Some(initial) = gear
                    .retained
                    .as_deref()
                    .and_then(|retained| retained.initial.as_ref())
                {
                    visit_expression(&initial.syntax, &mut use_name)?;
                }
            }
            BackStatement::Cord(cord) => visit_stages(&cord.stages, &mut use_name)?,
            BackStatement::MatchedRoute(route) => {
                use_name(&route.source.text, route.source.span)?;
                for arm in &route.arms {
                    if let MatchedRoutePattern::Guard { expected, .. } = &arm.pattern {
                        visit_expression(&expected.syntax, &mut use_name)?;
                    }
                    visit_stages(&arm.stages, &mut use_name)?;
                }
            }
            BackStatement::Pool(pool) => use_name(&pool.member_form.text, pool.member_form.span)?,
            BackStatement::LocalValue(value) => {
                visit_expression(&value.value.syntax, &mut use_name)?
            }
        }
    }
    Ok(())
}

fn visit_stages(
    stages: &[CordStage],
    visit: &mut impl FnMut(&str, Span) -> Result<(), SyntaxCheckDiagnostic>,
) -> Result<(), SyntaxCheckDiagnostic> {
    for stage in stages {
        match stage {
            CordStage::Reference(value) | CordStage::Glyph(value) => {
                visit(&value.text, value.span)?
            }
            CordStage::RelationalGlyph { operands, .. } => {
                for operand in operands {
                    visit(&operand.text, operand.span)?;
                }
            }
            CordStage::RelationalGear {
                operands,
                invocation,
                ..
            } => {
                for operand in operands {
                    visit(&operand.text, operand.span)?;
                }
                visit_invocation(invocation, visit)?;
            }
            CordStage::TerminalProjection { endpoint, .. } => {
                visit(endpoint.text.split('.').next().unwrap_or(""), endpoint.span)?
            }
            CordStage::Cancellation { gear, .. } => visit(&gear.text, gear.span)?,
            CordStage::When(expression)
            | CordStage::Literal(expression)
            | CordStage::PureExpression(expression) => visit_expression(&expression.syntax, visit)?,
            CordStage::InlineGear(invocation) => visit_invocation(invocation, visit)?,
            CordStage::StructuredSelector(_) => {}
        }
    }
    Ok(())
}

fn visit_invocation(
    invocation: &Invocation,
    visit: &mut impl FnMut(&str, Span) -> Result<(), SyntaxCheckDiagnostic>,
) -> Result<(), SyntaxCheckDiagnostic> {
    for argument in &invocation.arguments {
        match argument {
            Argument::Positional(value) | Argument::Named { value, .. } => {
                visit_expression(&value.syntax, visit)?
            }
        }
    }
    Ok(())
}

fn visit_expression(
    expression: &ExpressionSyntax,
    visit: &mut impl FnMut(&str, Span) -> Result<(), SyntaxCheckDiagnostic>,
) -> Result<(), SyntaxCheckDiagnostic> {
    match expression {
        ExpressionSyntax::Atomic(value) => visit(&value.text, value.span)?,
        ExpressionSyntax::Projection { value, .. }
        | ExpressionSyntax::Unary { operand: value, .. } => visit_expression(value, visit)?,
        ExpressionSyntax::Binary { left, right, .. } => {
            visit_expression(left, visit)?;
            visit_expression(right, visit)?;
        }
        ExpressionSyntax::Conditional {
            condition,
            when_true,
            when_false,
            ..
        } => {
            visit_expression(condition, visit)?;
            visit_expression(when_true, visit)?;
            visit_expression(when_false, visit)?;
        }
        ExpressionSyntax::Tuple { values, .. } | ExpressionSyntax::Collection { values, .. } => {
            for value in values {
                visit_expression(value, visit)?;
            }
        }
        ExpressionSyntax::Record { fields, .. } => {
            for field in fields {
                visit_expression(&field.value, visit)?;
            }
        }
        ExpressionSyntax::Variant { payload, .. } => visit_expression(payload, visit)?,
        ExpressionSyntax::SemanticCall {
            kind, arguments, ..
        } => {
            visit(&kind.text, kind.span)?;
            for argument in arguments {
                visit_expression(argument, visit)?;
            }
        }
        ExpressionSyntax::Input(_) => {}
    }
    Ok(())
}

fn rewrite_form(form: &mut FormSyntax, visible: &BTreeMap<String, LocalBinding>) {
    for parameter in &mut form.front.startup_parameters {
        if let Some(default) = &mut parameter.default {
            rewrite_expression(default, visible);
        }
    }
    for statement in &mut form.back {
        match statement {
            BackStatement::NamedGear(gear) => {
                rewrite_invocation(&mut gear.invocation, visible);
                if let Some(retained) = &mut gear.retained {
                    if let Some(initial) = &mut retained.initial {
                        rewrite_expression(initial, visible);
                    }
                }
            }
            BackStatement::Cord(cord) => rewrite_stages(&mut cord.stages, visible),
            BackStatement::MatchedRoute(route) => {
                for arm in &mut route.arms {
                    if let MatchedRoutePattern::Guard { expected, .. } = &mut arm.pattern {
                        rewrite_expression(expected, visible);
                    }
                    rewrite_stages(&mut arm.stages, visible);
                }
            }
            BackStatement::Pool(pool) => rewrite_name(&mut pool.member_form.text, visible),
            BackStatement::LocalValue(value) => rewrite_expression(&mut value.value, visible),
        }
    }
}

fn rewrite_stages(stages: &mut [CordStage], visible: &BTreeMap<String, LocalBinding>) {
    for stage in stages {
        match stage {
            CordStage::InlineGear(invocation) => rewrite_invocation(invocation, visible),
            CordStage::RelationalGear { invocation, .. } => rewrite_invocation(invocation, visible),
            CordStage::When(expression)
            | CordStage::Literal(expression)
            | CordStage::PureExpression(expression) => rewrite_expression(expression, visible),
            CordStage::Reference(_)
            | CordStage::Glyph(_)
            | CordStage::RelationalGlyph { .. }
            | CordStage::TerminalProjection { .. }
            | CordStage::Cancellation { .. }
            | CordStage::StructuredSelector(_) => {}
        }
    }
}

fn rewrite_invocation(invocation: &mut Invocation, visible: &BTreeMap<String, LocalBinding>) {
    if let Some(binding) = visible.get(&invocation.kind.text) {
        invocation.kind.text.clone_from(&binding.identity);
        for parameter in &binding.captured_types {
            invocation.arguments.push(Argument::Named {
                name: parameter.name.clone(),
                value: Expression {
                    text: parameter.name.text.clone(),
                    syntax: ExpressionSyntax::Atomic(parameter.name.clone()),
                    span: parameter.span,
                },
                span: parameter.span,
            });
        }
        for parameter in &binding.captured_kinds {
            invocation.arguments.push(Argument::Named {
                name: parameter.name.clone(),
                value: Expression {
                    text: parameter.name.text.clone(),
                    syntax: ExpressionSyntax::Atomic(parameter.name.clone()),
                    span: parameter.span,
                },
                span: parameter.span,
            });
        }
    }
    for argument in &mut invocation.arguments {
        match argument {
            Argument::Positional(value) | Argument::Named { value, .. } => {
                rewrite_expression(value, visible)
            }
        }
    }
}

fn rewrite_expression(expression: &mut Expression, visible: &BTreeMap<String, LocalBinding>) {
    rewrite_expression_syntax(&mut expression.syntax, visible);
}

fn rewrite_expression_syntax(
    expression: &mut ExpressionSyntax,
    visible: &BTreeMap<String, LocalBinding>,
) {
    match expression {
        ExpressionSyntax::Projection { value, .. }
        | ExpressionSyntax::Unary { operand: value, .. } => {
            rewrite_expression_syntax(value, visible)
        }
        ExpressionSyntax::Binary { left, right, .. } => {
            rewrite_expression_syntax(left, visible);
            rewrite_expression_syntax(right, visible);
        }
        ExpressionSyntax::Conditional {
            condition,
            when_true,
            when_false,
            ..
        } => {
            rewrite_expression_syntax(condition, visible);
            rewrite_expression_syntax(when_true, visible);
            rewrite_expression_syntax(when_false, visible);
        }
        ExpressionSyntax::Tuple { values, .. } | ExpressionSyntax::Collection { values, .. } => {
            for value in values {
                rewrite_expression_syntax(value, visible);
            }
        }
        ExpressionSyntax::Record { fields, .. } => {
            for field in fields {
                rewrite_expression_syntax(&mut field.value, visible);
            }
        }
        ExpressionSyntax::Variant { payload, .. } => rewrite_expression_syntax(payload, visible),
        ExpressionSyntax::SemanticCall {
            kind, arguments, ..
        } => {
            if let Some(binding) = visible.get(&kind.text) {
                kind.text.clone_from(&binding.identity);
            }
            for argument in arguments {
                rewrite_expression_syntax(argument, visible);
            }
        }
        ExpressionSyntax::Atomic(_) | ExpressionSyntax::Input(_) => {}
    }
}

fn rewrite_name(name: &mut String, visible: &BTreeMap<String, LocalBinding>) {
    if let Some(scoped) = visible.get(name) {
        name.clone_from(&scoped.identity);
    }
}

fn diagnostic(span: Span, message: String) -> SyntaxCheckDiagnostic {
    SyntaxCheckDiagnostic {
        code: "CND-FRM-061",
        span,
        message,
    }
}
