use crate::checked_syntax::{StartupCatalog, SyntaxCheckDiagnostic};
use crate::syntax::{
    Argument, BackStatement, CordStage, Expression, ExpressionSyntax, FormSyntax, Invocation,
    MatchedRoutePattern, SpannedText,
};
use crate::Span;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

const MAXIMUM_GENERIC_SPECIALIZATIONS: usize = 4_096;

/// Turns authored generic Form templates into exact ordinary Forms before the
/// existing checker constructs Fores. Type parameters are compile-time names;
/// they never become startup values or runtime ports.
pub(super) fn specialize_named_type_parameters(
    forms: Vec<FormSyntax>,
    catalog: &StartupCatalog,
) -> Result<Vec<FormSyntax>, SyntaxCheckDiagnostic> {
    let templates = forms
        .iter()
        .filter(|form| !type_parameters(form).is_empty())
        .map(|form| (form.name.text.clone(), form.clone()))
        .collect::<BTreeMap<_, _>>();
    if templates.is_empty() {
        return Ok(forms);
    }

    let mut concrete = forms
        .into_iter()
        .filter(|form| !templates.contains_key(&form.name.text))
        .collect::<Vec<_>>();
    let mut identities = concrete
        .iter()
        .map(|form| form.name.text.clone())
        .collect::<BTreeSet<_>>();
    let mut index = 0;
    while index < concrete.len() {
        let requests = rewrite_form_invocations(&mut concrete[index], &templates, catalog)?;
        index += 1;
        for request in requests {
            if identities.insert(request.identity.clone()) {
                if concrete.len() == MAXIMUM_GENERIC_SPECIALIZATIONS {
                    return Err(diagnostic(
                        concrete[index - 1].span,
                        format!(
                            "generic specialization exceeds the {MAXIMUM_GENERIC_SPECIALIZATIONS}-Form bound"
                        ),
                    ));
                }
                let template = templates
                    .get(&request.template)
                    .expect("specialization requests name a generic template");
                concrete.push(instantiate(template, &request));
            }
        }
    }
    Ok(concrete)
}

#[derive(Debug)]
struct SpecializationRequest {
    template: String,
    identity: String,
    substitutions: BTreeMap<String, String>,
}

fn type_parameters(form: &FormSyntax) -> &[crate::TypeParameter] {
    &form.front.type_parameters
}

fn rewrite_form_invocations(
    form: &mut FormSyntax,
    templates: &BTreeMap<String, FormSyntax>,
    catalog: &StartupCatalog,
) -> Result<Vec<SpecializationRequest>, SyntaxCheckDiagnostic> {
    let mut requests = Vec::new();
    for statement in &mut form.back {
        match statement {
            BackStatement::NamedGear(gear) => {
                rewrite_invocation(&mut gear.invocation, templates, catalog, &mut requests)?;
            }
            BackStatement::Cord(cord) => {
                rewrite_stages(&mut cord.stages, templates, catalog, &mut requests)?;
            }
            BackStatement::MatchedRoute(route) => {
                for arm in &mut route.arms {
                    rewrite_stages(&mut arm.stages, templates, catalog, &mut requests)?;
                }
            }
            BackStatement::Pool(_) | BackStatement::LocalValue(_) => {}
        }
    }
    Ok(requests)
}

fn rewrite_stages(
    stages: &mut [CordStage],
    templates: &BTreeMap<String, FormSyntax>,
    catalog: &StartupCatalog,
    requests: &mut Vec<SpecializationRequest>,
) -> Result<(), SyntaxCheckDiagnostic> {
    for stage in stages {
        if let CordStage::InlineGear(invocation) = stage {
            rewrite_invocation(invocation, templates, catalog, requests)?;
        }
    }
    Ok(())
}

fn rewrite_invocation(
    invocation: &mut Invocation,
    templates: &BTreeMap<String, FormSyntax>,
    catalog: &StartupCatalog,
    requests: &mut Vec<SpecializationRequest>,
) -> Result<(), SyntaxCheckDiagnostic> {
    let Some(template) = templates.get(&invocation.kind.text) else {
        return Ok(());
    };
    let parameters = type_parameters(template);
    let mut substitutions = BTreeMap::new();
    let mut retained_arguments = Vec::new();
    for argument in core::mem::take(&mut invocation.arguments) {
        let Argument::Named { name, value, .. } = &argument else {
            retained_arguments.push(argument);
            continue;
        };
        if !parameters
            .iter()
            .any(|parameter| parameter.name.text == name.text)
        {
            retained_arguments.push(argument);
            continue;
        }
        let source_type = atomic_type_name(value).ok_or_else(|| {
            diagnostic(
                value.span,
                format!(
                    "type argument '{}' must name one exact checked type",
                    name.text
                ),
            )
        })?;
        let canonical =
            crate::value_type::checked_value_kind(source_type, catalog).map_err(|_| {
                diagnostic(
                    value.span,
                    format!(
                        "type argument '{}' does not name an exact checked type",
                        name.text
                    ),
                )
            })?;
        if substitutions
            .insert(name.text.clone(), canonical.as_str().to_string())
            .is_some()
        {
            return Err(diagnostic(
                name.span,
                format!("type argument '{}' is bound more than once", name.text),
            ));
        }
    }
    for parameter in parameters {
        if !substitutions.contains_key(&parameter.name.text) {
            return Err(diagnostic(
                invocation.span,
                format!(
                    "generic Form '{}' requires exact type argument '{}'",
                    template.name.text, parameter.name.text
                ),
            ));
        }
    }
    invocation.arguments = retained_arguments;
    let identity = specialization_identity(&template.name.text, &substitutions);
    invocation.kind.text.clone_from(&identity);
    requests.push(SpecializationRequest {
        template: template.name.text.clone(),
        identity,
        substitutions,
    });
    Ok(())
}

fn atomic_type_name(expression: &Expression) -> Option<&str> {
    match &expression.syntax {
        ExpressionSyntax::Atomic(value) => Some(value.text.as_str()),
        _ => None,
    }
}

fn specialization_identity(template: &str, substitutions: &BTreeMap<String, String>) -> String {
    let arguments = substitutions
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join(",");
    format!("{template}[{arguments}]")
}

fn instantiate(template: &FormSyntax, request: &SpecializationRequest) -> FormSyntax {
    let mut form = template.clone();
    form.name.text.clone_from(&request.identity);
    form.front.type_parameters.clear();
    for parameter in &mut form.front.startup_parameters {
        substitute(&mut parameter.value_type, &request.substitutions);
    }
    for port in &mut form.front.runtime_ports {
        substitute(&mut port.value_type, &request.substitutions);
        if port.maximum_bytes.is_none() {
            port.maximum_bytes =
                crate::surface_parser::front::canonical_default_bound(&port.value_type.text);
        }
    }
    for statement in &mut form.back {
        match statement {
            BackStatement::NamedGear(gear) => {
                substitute_invocation_arguments(&mut gear.invocation, &request.substitutions);
                if let Some(retained) = &mut gear.retained {
                    substitute(&mut retained.value_type, &request.substitutions);
                }
            }
            BackStatement::Cord(cord) => {
                substitute_stages(&mut cord.stages, &request.substitutions)
            }
            BackStatement::MatchedRoute(route) => {
                for arm in &mut route.arms {
                    if let MatchedRoutePattern::Variant { value_type, .. }
                    | MatchedRoutePattern::Guard { value_type, .. } = &mut arm.pattern
                    {
                        substitute(value_type, &request.substitutions);
                    }
                    substitute_stages(&mut arm.stages, &request.substitutions);
                }
            }
            BackStatement::Pool(_) | BackStatement::LocalValue(_) => {}
        }
    }
    form
}

fn substitute_stages(stages: &mut [CordStage], substitutions: &BTreeMap<String, String>) {
    for stage in stages {
        match stage {
            CordStage::InlineGear(invocation) => {
                substitute_invocation_arguments(invocation, substitutions)
            }
            CordStage::StructuredSelector(selector) => match selector {
                crate::StructuredSelectorSyntax::Field { value_type, .. }
                | crate::StructuredSelectorSyntax::Index { value_type, .. }
                | crate::StructuredSelectorSyntax::Variant { value_type, .. } => {
                    substitute(value_type, substitutions)
                }
            },
            _ => {}
        }
    }
}

fn substitute_invocation_arguments(
    invocation: &mut Invocation,
    substitutions: &BTreeMap<String, String>,
) {
    for argument in &mut invocation.arguments {
        let value = match argument {
            Argument::Positional(value) | Argument::Named { value, .. } => value,
        };
        if let ExpressionSyntax::Atomic(atom) = &mut value.syntax {
            substitute(atom, substitutions);
            value.text.clone_from(&atom.text);
        }
    }
}

fn substitute(value_type: &mut SpannedText, substitutions: &BTreeMap<String, String>) {
    if let Some(concrete) = substitutions.get(&value_type.text) {
        value_type.text.clone_from(concrete);
    } else if let Some(inner) = value_type.text.strip_prefix('&') {
        if let Some(concrete) = substitutions.get(inner) {
            value_type.text = format!("&{concrete}");
        }
    }
}

fn diagnostic(span: Span, message: String) -> SyntaxCheckDiagnostic {
    SyntaxCheckDiagnostic {
        code: "CND-FRM-057",
        span,
        message,
    }
}
