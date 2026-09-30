use crate::prelude::*;
use crate::syntax::FormSyntax;
use crate::{Span, SyntaxCheckDiagnostic};
use alloc::collections::BTreeMap;
use alloc::format;

pub(super) fn validate(
    parameter: &crate::KindParameter,
    selected: &str,
    type_substitutions: &BTreeMap<String, String>,
    available_forms: &BTreeMap<String, FormSyntax>,
    catalog: &crate::StartupCatalog,
    span: Span,
) -> Result<(), SyntaxCheckDiagnostic> {
    if !crate::surface_lex::is_gear_name(selected) {
        return Err(diagnostic(
            span,
            format!(
                "behavior argument '{}' is not one exact Kind or source Form identity",
                parameter.name.text
            ),
        ));
    }
    let mut expected_syntax = FormSyntax {
        name: parameter.name.clone(),
        front: parameter.front.clone(),
        completion: crate::FormCompletionPolicy::Live,
        local_forms: Vec::new(),
        back: Vec::new(),
        expression_body: None,
        span: parameter.span,
    };
    for port in &mut expected_syntax.front.runtime_ports {
        super::substitution::value_type(&mut port.value_type, type_substitutions);
    }
    let expected = crate::value_type::checked_front(&expected_syntax, catalog)?;
    let actual = if let Some(form) = available_forms.get(selected) {
        if super::has_compile_time_parameters(form) {
            return Err(diagnostic(
                span,
                format!(
                    "behavior argument '{}' names unspecialized Form '{selected}'",
                    parameter.name.text
                ),
            ));
        }
        crate::value_type::checked_front(form, catalog)?
    } else {
        catalog.fore(selected).cloned().ok_or_else(|| {
            diagnostic(
                span,
                format!(
                    "behavior argument '{}' has no exact checked Fore for '{selected}'",
                    parameter.name.text
                ),
            )
        })?
    };
    if !runtime_fores_match(&expected, &actual) {
        return Err(diagnostic(
            span,
            format!(
                "behavior argument '{}' selects '{selected}' with an incompatible exact Fore",
                parameter.name.text
            ),
        ));
    }
    Ok(())
}

fn runtime_fores_match(
    expected: &conduit_core::CheckedFront,
    actual: &conduit_core::CheckedFront,
) -> bool {
    use conduit_core::FrontValueLocation;
    let runtime_contracts = |front: &conduit_core::CheckedFront| {
        front
            .value_contracts()
            .iter()
            .filter(|contract| !matches!(contract.location, FrontValueLocation::Startup(_)))
            .cloned()
            .collect::<Vec<_>>()
    };
    expected.inputs() == actual.inputs()
        && expected.outputs() == actual.outputs()
        && expected.shorthand() == actual.shorthand()
        && expected.resource_ports() == actual.resource_ports()
        && runtime_contracts(expected) == runtime_contracts(actual)
}

fn diagnostic(span: Span, message: String) -> SyntaxCheckDiagnostic {
    SyntaxCheckDiagnostic {
        code: "CND-FRM-057",
        span,
        message,
    }
}
