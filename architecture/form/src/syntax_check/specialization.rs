use crate::checked_syntax::{StartupCatalog, SyntaxCheckDiagnostic};
use crate::syntax::{
    Argument, BackStatement, CordStage, Expression, ExpressionSyntax, FormSyntax, Invocation,
    SpannedText,
};
use crate::Span;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

mod behavior;
mod instantiate;
mod substitution;

const MAXIMUM_GENERIC_SPECIALIZATIONS: usize = 4_096;

/// Turns authored generic Form templates into exact ordinary Forms before the
/// existing checker constructs Fores. Type and Kind/Form parameters are
/// compile-time names; they never become startup values or runtime ports.
pub(super) fn specialize_named_type_parameters(
    forms: Vec<FormSyntax>,
    catalog: &StartupCatalog,
) -> Result<Vec<FormSyntax>, SyntaxCheckDiagnostic> {
    let available_forms = forms
        .iter()
        .map(|form| (form.name.text.clone(), form.clone()))
        .collect::<BTreeMap<_, _>>();
    let templates = forms
        .iter()
        .filter(|form| has_compile_time_parameters(form))
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
        let requests =
            rewrite_form_invocations(&mut concrete[index], &templates, &available_forms, catalog)?;
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
                concrete.push(instantiate::exact(
                    template,
                    &request.identity,
                    &request.type_substitutions,
                    &request.behavior_substitutions,
                ));
            }
        }
    }
    Ok(concrete)
}

#[derive(Debug)]
struct SpecializationRequest {
    template: String,
    identity: String,
    type_substitutions: BTreeMap<String, String>,
    behavior_substitutions: BTreeMap<String, String>,
}

pub(super) fn has_compile_time_parameters(form: &FormSyntax) -> bool {
    !type_parameters(form).is_empty() || !form.front.kind_parameters.is_empty()
}

fn type_parameters(form: &FormSyntax) -> &[crate::TypeParameter] {
    &form.front.type_parameters
}

fn rewrite_form_invocations(
    form: &mut FormSyntax,
    templates: &BTreeMap<String, FormSyntax>,
    available_forms: &BTreeMap<String, FormSyntax>,
    catalog: &StartupCatalog,
) -> Result<Vec<SpecializationRequest>, SyntaxCheckDiagnostic> {
    let mut requests = Vec::new();
    let front_types = form
        .front
        .runtime_ports
        .iter()
        .map(|port| (port.name.text.clone(), port.value_type.text.clone()))
        .collect::<BTreeMap<_, _>>();
    for statement in &mut form.back {
        match statement {
            BackStatement::NamedGear(gear) => {
                rewrite_invocation(
                    &mut gear.invocation,
                    templates,
                    available_forms,
                    catalog,
                    &BTreeMap::new(),
                    &mut requests,
                )?;
            }
            BackStatement::Cord(cord) => {
                rewrite_stages(
                    &mut cord.stages,
                    templates,
                    available_forms,
                    catalog,
                    &front_types,
                    &mut requests,
                )?;
            }
            BackStatement::MatchedRoute(route) => {
                for arm in &mut route.arms {
                    rewrite_stages(
                        &mut arm.stages,
                        templates,
                        available_forms,
                        catalog,
                        &front_types,
                        &mut requests,
                    )?;
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
    available_forms: &BTreeMap<String, FormSyntax>,
    catalog: &StartupCatalog,
    front_types: &BTreeMap<String, String>,
    requests: &mut Vec<SpecializationRequest>,
) -> Result<(), SyntaxCheckDiagnostic> {
    for index in 0..stages.len() {
        let inferred =
            infer_from_adjacent_front_ports(stages, index, templates, catalog, front_types)?;
        if let CordStage::InlineGear(invocation) = &mut stages[index] {
            rewrite_invocation(
                invocation,
                templates,
                available_forms,
                catalog,
                &inferred,
                requests,
            )?;
        }
    }
    Ok(())
}

fn rewrite_invocation(
    invocation: &mut Invocation,
    templates: &BTreeMap<String, FormSyntax>,
    available_forms: &BTreeMap<String, FormSyntax>,
    catalog: &StartupCatalog,
    inferred: &BTreeMap<String, String>,
    requests: &mut Vec<SpecializationRequest>,
) -> Result<(), SyntaxCheckDiagnostic> {
    let Some(template) = templates.get(&invocation.kind.text) else {
        return Ok(());
    };
    let parameters = type_parameters(template);
    let behavior_parameters = &template.front.kind_parameters;
    let mut type_substitutions = BTreeMap::new();
    let mut behavior_substitutions = BTreeMap::new();
    let mut retained_arguments = Vec::new();
    for argument in core::mem::take(&mut invocation.arguments) {
        let Argument::Named { name, value, .. } = &argument else {
            retained_arguments.push(argument);
            continue;
        };
        let is_type = parameters
            .iter()
            .any(|parameter| parameter.name.text == name.text);
        let behavior = behavior_parameters
            .iter()
            .find(|parameter| parameter.name.text == name.text);
        if !is_type && behavior.is_none() {
            retained_arguments.push(argument);
            continue;
        }
        if behavior.is_some() {
            let selected = crate::surface_lex::is_gear_name(value.text.trim())
                .then(|| value.text.trim())
                .ok_or_else(|| {
                    diagnostic(
                        value.span,
                        format!(
                            "behavior argument '{}' must name one exact installed Kind or checked source Form",
                            name.text
                        ),
                    )
                })?;
            if behavior_substitutions
                .insert(name.text.clone(), selected.to_string())
                .is_some()
            {
                return Err(diagnostic(
                    name.span,
                    format!("behavior argument '{}' is bound more than once", name.text),
                ));
            }
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
        if type_substitutions
            .insert(name.text.clone(), canonical.as_str().to_string())
            .is_some()
        {
            return Err(diagnostic(
                name.span,
                format!("type argument '{}' is bound more than once", name.text),
            ));
        }
    }
    for (name, concrete) in inferred {
        if let Some(explicit) = type_substitutions.get(name) {
            if explicit != concrete {
                return Err(diagnostic(
                    invocation.span,
                    format!(
                        "type argument '{name}' is '{explicit}' but connected ports require '{concrete}'"
                    ),
                ));
            }
        } else {
            type_substitutions.insert(name.clone(), concrete.clone());
        }
    }
    for parameter in parameters {
        if !type_substitutions.contains_key(&parameter.name.text) {
            return Err(diagnostic(
                invocation.span,
                format!(
                    "generic Form '{}' requires exact type argument '{}'",
                    template.name.text, parameter.name.text
                ),
            ));
        }
    }
    for parameter in behavior_parameters {
        let selected = behavior_substitutions
            .get(&parameter.name.text)
            .ok_or_else(|| {
                diagnostic(
                    invocation.span,
                    format!(
                        "generic Form '{}' requires exact behavior argument '{}'",
                        template.name.text, parameter.name.text
                    ),
                )
            })?;
        behavior::validate(
            parameter,
            selected,
            &type_substitutions,
            available_forms,
            catalog,
            invocation.span,
        )?;
    }
    invocation.arguments = retained_arguments;
    let identity = specialization_identity(
        &template.name.text,
        &type_substitutions,
        &behavior_substitutions,
    );
    invocation.kind.text.clone_from(&identity);
    requests.push(SpecializationRequest {
        template: template.name.text.clone(),
        identity,
        type_substitutions,
        behavior_substitutions,
    });
    Ok(())
}

fn infer_from_adjacent_front_ports(
    stages: &[CordStage],
    index: usize,
    templates: &BTreeMap<String, FormSyntax>,
    catalog: &StartupCatalog,
    front_types: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, SyntaxCheckDiagnostic> {
    let CordStage::InlineGear(invocation) = &stages[index] else {
        return Ok(BTreeMap::new());
    };
    let Some(template) = templates.get(&invocation.kind.text) else {
        return Ok(BTreeMap::new());
    };
    let parameters = type_parameters(template)
        .iter()
        .map(|parameter| parameter.name.text.as_str())
        .collect::<BTreeSet<_>>();
    let inputs = template
        .front
        .runtime_ports
        .iter()
        .filter(|port| port.direction == crate::RuntimePortDirection::Input)
        .collect::<Vec<_>>();
    let outputs = template
        .front
        .runtime_ports
        .iter()
        .filter(|port| port.direction == crate::RuntimePortDirection::Output)
        .collect::<Vec<_>>();
    let mut inferred = BTreeMap::new();
    if inputs.len() == 1 {
        if let Some(CordStage::Reference(reference)) = index.checked_sub(1).map(|i| &stages[i]) {
            if let Some(concrete) = front_types.get(&reference.text) {
                infer_parameter(
                    &inputs[0].value_type,
                    concrete,
                    &parameters,
                    catalog,
                    &mut inferred,
                    invocation.span,
                )?;
            }
        }
    }
    if outputs.len() == 1 {
        if let Some(CordStage::Reference(reference)) = stages.get(index + 1) {
            if let Some(concrete) = front_types.get(&reference.text) {
                infer_parameter(
                    &outputs[0].value_type,
                    concrete,
                    &parameters,
                    catalog,
                    &mut inferred,
                    invocation.span,
                )?;
            }
        }
    }
    Ok(inferred)
}

fn infer_parameter(
    generic: &SpannedText,
    concrete: &str,
    parameters: &BTreeSet<&str>,
    catalog: &StartupCatalog,
    inferred: &mut BTreeMap<String, String>,
    span: Span,
) -> Result<(), SyntaxCheckDiagnostic> {
    if !parameters.contains(generic.text.as_str()) {
        return Ok(());
    }
    let concrete = crate::value_type::checked_value_kind(concrete, catalog).map_err(|_| {
        diagnostic(
            span,
            format!("connected port type '{concrete}' is not one exact checked type"),
        )
    })?;
    let concrete = concrete.as_str().to_string();
    if inferred
        .insert(generic.text.clone(), concrete.clone())
        .is_some_and(|prior| prior != concrete)
    {
        return Err(diagnostic(
            span,
            format!(
                "connected ports require conflicting types for '{}'",
                generic.text
            ),
        ));
    }
    Ok(())
}

fn atomic_type_name(expression: &Expression) -> Option<&str> {
    match &expression.syntax {
        ExpressionSyntax::Atomic(value) => Some(value.text.as_str()),
        _ => None,
    }
}

fn specialization_identity(
    template: &str,
    type_substitutions: &BTreeMap<String, String>,
    behavior_substitutions: &BTreeMap<String, String>,
) -> String {
    let arguments = type_substitutions
        .iter()
        .chain(behavior_substitutions)
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join(",");
    format!("{template}[{arguments}]")
}

fn diagnostic(span: Span, message: String) -> SyntaxCheckDiagnostic {
    SyntaxCheckDiagnostic {
        code: "CND-FRM-057",
        span,
        message,
    }
}
