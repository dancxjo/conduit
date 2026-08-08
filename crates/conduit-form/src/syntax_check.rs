use crate::checked_syntax::{
    CanonicalStartupValue, CheckedCanonicalCell, CheckedCanonicalCord, CheckedCanonicalForm,
    CheckedCordStage, CheckedStartupBinding, CheckedStartupParameter, CheckedSyntaxDocument,
    OperationSignature, StartupCatalog, StartupParameterSignature, SyntaxCheckDiagnostic,
    SyntaxCheckError,
};
use crate::syntax::{Argument, BackStatement, CordStage, FormSyntax, Invocation, SyntaxDocument};
use crate::syntax_identity::{canonical_cell, canonical_cord, checked_identity};
use crate::{hash_string, Span};
use conduit_core::SourceDocumentId;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn check_document(
    document: &SyntaxDocument,
    catalog: &StartupCatalog,
) -> Result<CheckedSyntaxDocument, SyntaxCheckDiagnostic> {
    if let Some(diagnostic) = document.diagnostics.first() {
        return Err(SyntaxCheckDiagnostic {
            code: diagnostic.code,
            span: diagnostic.span,
            message: diagnostic.message.clone(),
        });
    }
    let form_signatures = form_signatures(&document.forms)?;
    let mut forms = Vec::with_capacity(document.forms.len());
    for form in &document.forms {
        forms.push(check_form(form, catalog, &form_signatures)?);
    }
    forms.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(CheckedSyntaxDocument {
        source_document_id: SourceDocumentId::from(hash_string(&format!(
            "canonical-source:{}",
            document.round_trip()
        ))),
        forms,
    })
}

fn form_signatures(
    forms: &[FormSyntax],
) -> Result<BTreeMap<String, OperationSignature>, SyntaxCheckDiagnostic> {
    let mut signatures = BTreeMap::new();
    for form in forms {
        if signatures.contains_key(&form.name.text) {
            return Err(SyntaxCheckError::DuplicateImmutable(form.name.text.clone())
                .diagnostic(form.name.span));
        }
        let mut names = BTreeSet::new();
        let mut startup_parameters = Vec::new();
        for parameter in &form.face.startup_parameters {
            if !names.insert(parameter.name.text.clone()) {
                return Err(
                    SyntaxCheckError::DuplicateImmutable(parameter.name.text.clone())
                        .diagnostic(parameter.span),
                );
            }
            startup_parameters.push(StartupParameterSignature {
                name: parameter.name.text.clone(),
                value_type: parameter.value_type.text.clone(),
                default: parameter.default.as_ref().map(|value| value.text.clone()),
            });
        }
        signatures.insert(
            form.name.text.clone(),
            OperationSignature {
                operation: form.name.text.clone(),
                startup_parameters,
            },
        );
    }
    Ok(signatures)
}

fn check_form(
    form: &FormSyntax,
    catalog: &StartupCatalog,
    form_signatures: &BTreeMap<String, OperationSignature>,
) -> Result<CheckedCanonicalForm, SyntaxCheckDiagnostic> {
    let signature = form_signatures
        .get(&form.name.text)
        .expect("every parsed form has a derived signature");
    let parameters = checked_parameters(signature, form.name.span)?;
    let parameter_names = signature
        .startup_parameters
        .iter()
        .map(|parameter| parameter.name.clone())
        .collect::<BTreeSet<_>>();
    let mut runtime_names = BTreeSet::new();
    for port in &form.face.runtime_ports {
        if !runtime_names.insert(port.name.text.clone())
            || parameter_names.contains(&port.name.text)
        {
            return Err(
                SyntaxCheckError::AmbiguousFaceName(port.name.text.clone()).diagnostic(port.span)
            );
        }
    }
    let face_names = parameter_names
        .union(&runtime_names)
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut locals = BTreeMap::new();
    let mut named_cells = BTreeSet::new();
    for statement in &form.back {
        match statement {
            BackStatement::LocalValue(local) => {
                if parameter_names.contains(&local.name.text)
                    || runtime_names.contains(&local.name.text)
                {
                    return Err(
                        SyntaxCheckError::DuplicateImmutable(local.name.text.clone())
                            .diagnostic(local.span),
                    );
                }
                if locals.insert(local.name.text.clone(), local).is_some() {
                    return Err(
                        SyntaxCheckError::DuplicateImmutable(local.name.text.clone())
                            .diagnostic(local.span),
                    );
                }
            }
            BackStatement::NamedCell(cell) => {
                if face_names.contains(&cell.name.text) {
                    return Err(SyntaxCheckError::AmbiguousFaceName(cell.name.text.clone())
                        .diagnostic(cell.span));
                }
                if !named_cells.insert(cell.name.text.clone()) {
                    return Err(SyntaxCheckError::DuplicateCell(cell.name.text.clone())
                        .diagnostic(cell.span));
                }
            }
            BackStatement::Cord(_) => {}
        }
    }

    let mut resolver = Resolver::new(locals, parameter_names, runtime_names);
    let local_names = resolver.locals.keys().cloned().collect::<Vec<_>>();
    let mut local_values = Vec::with_capacity(local_names.len());
    for name in local_names {
        let local = resolver.locals[&name];
        let value = resolver
            .resolve_name(&name)
            .map_err(|error| error.diagnostic(local.span))?;
        local_values.push((name, value));
    }

    let mut cells = Vec::new();
    let mut cords = Vec::new();
    for statement in &form.back {
        match statement {
            BackStatement::NamedCell(cell) => cells.push(check_invocation(
                Some(cell.name.text.clone()),
                &cell.invocation,
                catalog,
                form_signatures,
                &mut resolver,
            )?),
            BackStatement::Cord(cord) => {
                let mut stages = Vec::new();
                for stage in &cord.stages {
                    match stage {
                        CordStage::Reference(reference) => {
                            stages.push(CheckedCordStage::Reference(reference.text.clone()));
                        }
                        CordStage::InlineCell(invocation) => {
                            let cell = check_invocation(
                                None,
                                invocation,
                                catalog,
                                form_signatures,
                                &mut resolver,
                            )?;
                            stages.push(CheckedCordStage::InlineCell(cell.clone()));
                            cells.push(cell);
                        }
                    }
                }
                cords.push(CheckedCanonicalCord { stages });
            }
            BackStatement::LocalValue(_) => {}
        }
    }
    cells.sort_by_key(canonical_cell);
    cords.sort_by_key(canonical_cord);
    local_values.sort_by(|left, right| left.0.cmp(&right.0));
    let checked_form_id = checked_identity(
        &form.name.text,
        &parameters,
        &form.face.runtime_ports,
        form.face.shorthand.as_ref().map(|pair| {
            (
                pair.input_port.text.as_str(),
                pair.output_port.text.as_str(),
            )
        }),
        &cells,
        &cords,
    );
    Ok(CheckedCanonicalForm {
        checked_form_id,
        name: form.name.text.clone(),
        startup_parameters: parameters,
        runtime_ports: form.face.runtime_ports.clone(),
        shorthand: form
            .face
            .shorthand
            .as_ref()
            .map(|pair| (pair.input_port.text.clone(), pair.output_port.text.clone())),
        local_values,
        cells,
        cords,
    })
}

fn checked_parameters(
    signature: &OperationSignature,
    span: Span,
) -> Result<Vec<CheckedStartupParameter>, SyntaxCheckDiagnostic> {
    let mut values = vec![None; signature.startup_parameters.len()];
    let mut checked = Vec::with_capacity(signature.startup_parameters.len());
    for (index, parameter) in signature.startup_parameters.iter().enumerate() {
        let default = if parameter.default.is_some() {
            Some(
                resolve_bound_value(index, &mut values, signature, &mut BTreeSet::new())
                    .map_err(|error| error.diagnostic(span))?,
            )
        } else {
            None
        };
        checked.push(CheckedStartupParameter {
            name: parameter.name.clone(),
            value_type: parameter.value_type.clone(),
            default,
        });
    }
    Ok(checked)
}

fn check_invocation(
    name: Option<String>,
    invocation: &Invocation,
    catalog: &StartupCatalog,
    form_signatures: &BTreeMap<String, OperationSignature>,
    resolver: &mut Resolver<'_>,
) -> Result<CheckedCanonicalCell, SyntaxCheckDiagnostic> {
    let signature = form_signatures
        .get(&invocation.operation.text)
        .or_else(|| catalog.get(&invocation.operation.text))
        .ok_or_else(|| {
            SyntaxCheckError::UnsupportedOperation(invocation.operation.text.clone())
                .diagnostic(invocation.operation.span)
        })?;
    let mut values = vec![None; signature.startup_parameters.len()];
    let mut positional_count = 0usize;
    for argument in &invocation.arguments {
        match argument {
            Argument::Positional(expression) => {
                if positional_count >= signature.startup_parameters.len() {
                    return Err(
                        SyntaxCheckError::TooManyPositional(signature.operation.clone())
                            .diagnostic(expression.span),
                    );
                }
                values[positional_count] = Some(
                    resolver
                        .resolve_expression(&expression.text)
                        .map_err(|error| error.diagnostic(expression.span))?,
                );
                positional_count += 1;
            }
            Argument::Named { name, value, span } => {
                let index = signature
                    .startup_parameters
                    .iter()
                    .position(|parameter| parameter.name == name.text)
                    .ok_or_else(|| {
                        SyntaxCheckError::UnknownParameter(name.text.clone()).diagnostic(name.span)
                    })?;
                if values[index].is_some() {
                    let error = if index < positional_count {
                        SyntaxCheckError::PositionalNamedDuplicate(name.text.clone())
                    } else {
                        SyntaxCheckError::ConflictingArgument(name.text.clone())
                    };
                    return Err(error.diagnostic(*span));
                }
                values[index] = Some(
                    resolver
                        .resolve_expression(&value.text)
                        .map_err(|error| error.diagnostic(value.span))?,
                );
            }
        }
    }
    let mut startup_bindings = Vec::with_capacity(signature.startup_parameters.len());
    for (index, parameter) in signature.startup_parameters.iter().enumerate() {
        let value = resolve_bound_value(index, &mut values, signature, &mut BTreeSet::new())
            .map_err(|error| error.diagnostic(invocation.span))?;
        startup_bindings.push(CheckedStartupBinding {
            name: parameter.name.clone(),
            value_type: parameter.value_type.clone(),
            value,
        });
    }
    Ok(CheckedCanonicalCell {
        name,
        operation: signature.operation.clone(),
        startup_parameters: signature.startup_parameters.clone(),
        startup_bindings,
        source_span: invocation.span,
    })
}

fn resolve_bound_value(
    index: usize,
    values: &mut [Option<CanonicalStartupValue>],
    signature: &OperationSignature,
    visiting: &mut BTreeSet<usize>,
) -> Result<CanonicalStartupValue, SyntaxCheckError> {
    if let Some(value) = &values[index] {
        return Ok(value.clone());
    }
    let parameter = &signature.startup_parameters[index];
    let default = parameter
        .default
        .as_deref()
        .ok_or_else(|| SyntaxCheckError::MissingParameter(parameter.name.clone()))?;
    if !visiting.insert(index) {
        return Err(SyntaxCheckError::DependencyCycle(parameter.name.clone()));
    }
    let value = if let Some(reference) = signature
        .startup_parameters
        .iter()
        .position(|candidate| candidate.name == default)
    {
        if values[reference].is_some() || signature.startup_parameters[reference].default.is_some()
        {
            resolve_bound_value(reference, values, signature, visiting)?
        } else {
            CanonicalStartupValue::FormParameter(default.to_string())
        }
    } else {
        CanonicalStartupValue::Literal(default.to_string())
    };
    visiting.remove(&index);
    values[index] = Some(value.clone());
    Ok(value)
}

struct Resolver<'a> {
    locals: BTreeMap<String, &'a crate::LocalValue>,
    parameters: BTreeSet<String>,
    runtime_ports: BTreeSet<String>,
    resolved: BTreeMap<String, CanonicalStartupValue>,
    visiting: BTreeSet<String>,
}

impl<'a> Resolver<'a> {
    fn new(
        locals: BTreeMap<String, &'a crate::LocalValue>,
        parameters: BTreeSet<String>,
        runtime_ports: BTreeSet<String>,
    ) -> Self {
        Self {
            locals,
            parameters,
            runtime_ports,
            resolved: BTreeMap::new(),
            visiting: BTreeSet::new(),
        }
    }

    fn resolve_name(&mut self, name: &str) -> Result<CanonicalStartupValue, SyntaxCheckError> {
        if let Some(value) = self.resolved.get(name) {
            return Ok(value.clone());
        }
        if !self.visiting.insert(name.to_string()) {
            return Err(SyntaxCheckError::DependencyCycle(name.to_string()));
        }
        let expression = self.locals[name].value.text.clone();
        let value = self.resolve_expression(&expression)?;
        self.visiting.remove(name);
        self.resolved.insert(name.to_string(), value.clone());
        Ok(value)
    }

    fn resolve_expression(
        &mut self,
        expression: &str,
    ) -> Result<CanonicalStartupValue, SyntaxCheckError> {
        if self.locals.contains_key(expression) {
            self.resolve_name(expression)
        } else if self.runtime_ports.contains(expression) {
            Err(SyntaxCheckError::RuntimeAsStartup(expression.to_string()))
        } else if self.parameters.contains(expression) {
            Ok(CanonicalStartupValue::FormParameter(expression.to_string()))
        } else if is_atomic_literal(expression) {
            Ok(CanonicalStartupValue::Literal(expression.to_string()))
        } else if let Some(runtime) = self
            .runtime_ports
            .iter()
            .find(|name| contains_identifier(expression, name))
        {
            Err(SyntaxCheckError::RuntimeAsStartup(runtime.clone()))
        } else {
            Err(SyntaxCheckError::UnsupportedExpression(
                expression.to_string(),
            ))
        }
    }
}

fn is_atomic_literal(expression: &str) -> bool {
    let quoted = (expression.starts_with('"') && expression.ends_with('"'))
        || (expression.starts_with('\'') && expression.ends_with('\''));
    quoted
        || !expression.is_empty()
            && !expression.chars().any(|character| {
                character.is_whitespace()
                    || matches!(
                        character,
                        '(' | ')' | '[' | ']' | '{' | '}' | ',' | '+' | '*' | '/'
                    )
            })
}

fn contains_identifier(expression: &str, name: &str) -> bool {
    expression
        .split(|character: char| !(character.is_alphanumeric() || matches!(character, '_' | '-')))
        .any(|candidate| candidate == name)
}
