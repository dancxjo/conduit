use crate::checked_syntax::{
    CanonicalStartupValue, CheckedCanonicalCord, CheckedCanonicalForm, CheckedCanonicalGear,
    CheckedCordStage, CheckedStartupBinding, CheckedStartupParameter, CheckedSyntaxDocument,
    KindSignature, StartupCatalog, StartupParameterSignature, SyntaxCheckDiagnostic,
    SyntaxCheckError,
};
use crate::hash_string;
use crate::prelude::*;
use crate::syntax::{
    Argument, BackStatement, CordStage, FormSyntax, Invocation, MatchedRoute, MatchedRoutePattern,
    SyntaxDocument,
};
use crate::syntax_identity::{canonical_cord, canonical_gear, checked_identity};
use alloc::collections::{BTreeMap, BTreeSet};
use conduit_core::{
    CheckedFront, SourceDocumentId, StructuredInfoTypeShape, StructuredSelector,
    UnmatchedVariantDisposition,
};

mod resolution;
mod shared_pool;
mod structured_selector;
use resolution::{is_atomic_literal, Resolver};
use shared_pool::{check_pool_declarations, checked_pool};

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
    let mut form_fronts = BTreeMap::new();
    for form in &document.forms {
        form_fronts.insert(
            form.name.text.clone(),
            crate::value_type::checked_front(form, catalog)?,
        );
    }
    let mut forms = Vec::with_capacity(document.forms.len());
    for form in &document.forms {
        forms.push(check_form(form, catalog, &form_signatures, &form_fronts)?);
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
) -> Result<BTreeMap<String, KindSignature>, SyntaxCheckDiagnostic> {
    let mut signatures = BTreeMap::new();
    for form in forms {
        if signatures.contains_key(&form.name.text) {
            return Err(SyntaxCheckError::DuplicateImmutable(form.name.text.clone())
                .diagnostic(form.name.span));
        }
        let mut names = BTreeSet::new();
        let mut startup_parameters = Vec::new();
        for parameter in &form.front.startup_parameters {
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
            KindSignature {
                kind: form.name.text.clone(),
                startup_parameters,
            },
        );
    }
    Ok(signatures)
}

fn check_form(
    form: &FormSyntax,
    catalog: &StartupCatalog,
    form_signatures: &BTreeMap<String, KindSignature>,
    form_fronts: &BTreeMap<String, CheckedFront>,
) -> Result<CheckedCanonicalForm, SyntaxCheckDiagnostic> {
    let signature = form_signatures
        .get(&form.name.text)
        .expect("every parsed form has a derived signature");
    let parameters = checked_parameters(signature, catalog, form)?;
    let parameter_names = signature
        .startup_parameters
        .iter()
        .map(|parameter| parameter.name.clone())
        .collect::<BTreeSet<_>>();
    let mut runtime_names = BTreeSet::new();
    for port in &form.front.runtime_ports {
        if !runtime_names.insert(port.name.text.clone())
            || parameter_names.contains(&port.name.text)
        {
            return Err(
                SyntaxCheckError::AmbiguousFrontName(port.name.text.clone()).diagnostic(port.span)
            );
        }
    }
    let front_names = parameter_names
        .union(&runtime_names)
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut locals = BTreeMap::new();
    let mut named_gears = BTreeSet::new();
    let pool_names = check_pool_declarations(form, &front_names, form_fronts)?;
    for statement in &form.back {
        match statement {
            BackStatement::LocalValue(local) => {
                if parameter_names.contains(&local.name.text)
                    || runtime_names.contains(&local.name.text)
                    || pool_names.contains(&local.name.text)
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
            BackStatement::NamedGear(gear) => {
                if front_names.contains(&gear.name.text) || pool_names.contains(&gear.name.text) {
                    return Err(SyntaxCheckError::AmbiguousFrontName(gear.name.text.clone())
                        .diagnostic(gear.span));
                }
                if !named_gears.insert(gear.name.text.clone()) {
                    return Err(SyntaxCheckError::DuplicateGear(gear.name.text.clone())
                        .diagnostic(gear.span));
                }
            }
            BackStatement::Pool(pool) => {
                if named_gears.contains(&pool.name.text) {
                    return Err(SyntaxCheckError::DuplicateGear(pool.name.text.clone())
                        .diagnostic(pool.span));
                }
            }
            BackStatement::Cord(_) | BackStatement::MatchedRoute(_) => {}
        }
    }

    let mut resolver = Resolver::new(locals, parameter_names, runtime_names, pool_names);
    let mut gears = Vec::new();
    let mut cords = Vec::new();
    let mut pools = Vec::new();
    for statement in &form.back {
        match statement {
            BackStatement::NamedGear(gear) => gears.push(check_invocation(
                Some(gear.name.text.clone()),
                &gear.invocation,
                catalog,
                form_signatures,
                &mut resolver,
            )?),
            BackStatement::Cord(cord) => {
                let stages = check_cord_stages(
                    &cord.stages,
                    catalog,
                    form_signatures,
                    &mut resolver,
                    &mut gears,
                )?;
                cords.push(CheckedCanonicalCord { stages });
            }
            BackStatement::MatchedRoute(route) => cords.extend(check_matched_route(
                route,
                catalog,
                form_signatures,
                &mut resolver,
                &mut gears,
            )?),
            BackStatement::Pool(pool) => pools.push(checked_pool(pool, form_fronts)),
            BackStatement::LocalValue(_) => {}
        }
    }
    let local_names = resolver.locals.keys().cloned().collect::<Vec<_>>();
    let mut local_values = Vec::with_capacity(local_names.len());
    for name in local_names {
        let local = resolver.locals[&name];
        let value = resolver
            .resolve_name(&name, None)
            .map_err(|error| error.diagnostic(local.span))?;
        local_values.push((name, value));
    }
    gears.sort_by_key(canonical_gear);
    cords.sort_by_key(canonical_cord);
    pools.sort_by(|left, right| left.name.cmp(&right.name));
    local_values.sort_by(|left, right| left.0.cmp(&right.0));
    let runtime_front = form_fronts
        .get(&form.name.text)
        .expect("every parsed form has a checked front")
        .clone();
    let checked_form_id = checked_identity(
        (&form.name.text, form.completion),
        &parameters,
        &runtime_front,
        form.front.shorthand.as_ref().map(|pair| {
            (
                pair.input_port.text.as_str(),
                pair.output_port.text.as_str(),
            )
        }),
        &gears,
        &cords,
        &pools,
    );
    Ok(CheckedCanonicalForm {
        checked_form_id,
        name: form.name.text.clone(),
        completion: form.completion,
        startup_parameters: parameters,
        runtime_ports: form.front.runtime_ports.clone(),
        runtime_front,
        shorthand: form
            .front
            .shorthand
            .as_ref()
            .map(|pair| (pair.input_port.text.clone(), pair.output_port.text.clone())),
        local_values,
        pools,
        gears,
        cords,
    })
}

fn check_cord_stages(
    source: &[CordStage],
    catalog: &StartupCatalog,
    form_signatures: &BTreeMap<String, KindSignature>,
    resolver: &mut Resolver<'_>,
    gears: &mut Vec<CheckedCanonicalGear>,
) -> Result<Vec<CheckedCordStage>, SyntaxCheckDiagnostic> {
    let mut stages = Vec::with_capacity(source.len());
    for stage in source {
        match stage {
            CordStage::Reference(reference) => {
                stages.push(CheckedCordStage::Reference(reference.text.clone()));
            }
            CordStage::InlineGear(invocation) => {
                let gear = check_invocation(None, invocation, catalog, form_signatures, resolver)?;
                stages.push(CheckedCordStage::InlineGear(gear.clone()));
                gears.push(gear);
            }
            CordStage::Literal(expression) => {
                let value = resolver
                    .resolve_expression(expression, None)
                    .map_err(|error| error.diagnostic(expression.span))?;
                if !matches!(value, CanonicalStartupValue::Literal(_))
                    || crate::text_value::parse_quoted_text(&expression.text).is_none()
                {
                    return Err(
                        SyntaxCheckError::UnsupportedExpression(expression.text.clone())
                            .diagnostic(expression.span),
                    );
                }
                stages.push(CheckedCordStage::Literal {
                    value,
                    source_span: expression.span,
                });
            }
            CordStage::StructuredSelector(selector) => {
                stages.push(CheckedCordStage::StructuredSelector {
                    selector: structured_selector::check(selector, catalog)?,
                    source_span: selector.span(),
                });
            }
        }
    }
    Ok(stages)
}

fn check_matched_route(
    route: &MatchedRoute,
    catalog: &StartupCatalog,
    form_signatures: &BTreeMap<String, KindSignature>,
    resolver: &mut Resolver<'_>,
    gears: &mut Vec<CheckedCanonicalGear>,
) -> Result<Vec<CheckedCanonicalCord>, SyntaxCheckDiagnostic> {
    let mut route_type = None;
    let mut seen = BTreeSet::new();
    for (index, arm) in route.arms.iter().enumerate() {
        match &arm.pattern {
            MatchedRoutePattern::Otherwise(span) => {
                if index + 1 != route.arms.len() {
                    return Err(route_diagnostic(*span, "otherwise track must be final"));
                }
                return Err(route_diagnostic(
                    *span,
                    "closed variant routing requires explicit exhaustive tracks",
                ));
            }
            MatchedRoutePattern::Variant {
                value_type,
                tag,
                span,
            } => {
                match route_type {
                    Some(current) if current != value_type.text => {
                        return Err(route_diagnostic(
                            *span,
                            "all tracks in one matched route must use the same variant type",
                        ));
                    }
                    None => route_type = Some(value_type.text.as_str()),
                    Some(_) => {}
                }
                if !seen.insert(tag.text.as_str()) {
                    return Err(route_diagnostic(tag.span, "duplicate matched route track"));
                }
            }
        }
    }
    let type_name = route_type.ok_or_else(|| {
        route_diagnostic(route.span, "matched routing requires typed variant tracks")
    })?;
    let value_type = catalog
        .structured_type(type_name)
        .cloned()
        .ok_or_else(|| route_diagnostic(route.span, "unknown matched route variant type"))?;
    let StructuredInfoTypeShape::Variant { cases, .. } = value_type.shape() else {
        return Err(route_diagnostic(
            route.span,
            "matched routing type must be a closed structured variant",
        ));
    };
    let expected = cases.iter().map(|case| case.tag()).collect::<BTreeSet<_>>();
    if seen != expected {
        return Err(route_diagnostic(
            route.span,
            "closed variant routing must name every case exactly once",
        ));
    }

    let mut cords = Vec::with_capacity(route.arms.len());
    for arm in &route.arms {
        let MatchedRoutePattern::Variant { tag, span, .. } = &arm.pattern else {
            unreachable!("closed variant routing admitted only explicit cases")
        };
        let selector = StructuredSelector::variant(
            value_type.clone(),
            tag.text.clone(),
            UnmatchedVariantDisposition::Drop,
        )
        .map_err(|_| route_diagnostic(*span, "invalid matched route variant track"))?;
        let mut stages = vec![
            CheckedCordStage::Reference(route.source.text.clone()),
            CheckedCordStage::StructuredSelector {
                selector,
                source_span: *span,
            },
        ];
        let carried = arm.stages.first().is_some_and(
            |stage| matches!(stage, CordStage::Reference(reference) if reference.text == "_"),
        );
        if let Some(stage) =
            arm.stages.iter().skip(usize::from(carried)).find(
                |stage| matches!(stage, CordStage::Reference(reference) if reference.text == "_"),
            )
        {
            let CordStage::Reference(reference) = stage else {
                unreachable!()
            };
            return Err(route_diagnostic(
                reference.span,
                "carried value '_' must be the first stage of a matched track",
            ));
        }
        stages.extend(check_cord_stages(
            &arm.stages[usize::from(carried)..],
            catalog,
            form_signatures,
            resolver,
            gears,
        )?);
        cords.push(CheckedCanonicalCord { stages });
    }
    Ok(cords)
}

fn route_diagnostic(span: crate::Span, message: &str) -> SyntaxCheckDiagnostic {
    SyntaxCheckDiagnostic {
        code: "CND-FRM-054",
        span,
        message: message.into(),
    }
}

fn checked_parameters(
    signature: &KindSignature,
    catalog: &StartupCatalog,
    form: &FormSyntax,
) -> Result<Vec<CheckedStartupParameter>, SyntaxCheckDiagnostic> {
    let mut values = vec![None; signature.startup_parameters.len()];
    let mut checked = Vec::with_capacity(signature.startup_parameters.len());
    for (index, parameter) in signature.startup_parameters.iter().enumerate() {
        let default = if parameter.default.is_some() {
            Some(
                resolve_bound_value(index, &mut values, signature, catalog, &mut BTreeSet::new())
                    .map_err(|error| {
                    let parameter = &form.front.startup_parameters[index];
                    error.diagnostic(
                        parameter
                            .default
                            .as_ref()
                            .map_or(parameter.span, |value| value.span),
                    )
                })?,
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
    form_signatures: &BTreeMap<String, KindSignature>,
    resolver: &mut Resolver<'_>,
) -> Result<CheckedCanonicalGear, SyntaxCheckDiagnostic> {
    let signature = form_signatures
        .get(&invocation.kind.text)
        .or_else(|| catalog.get(&invocation.kind.text))
        .ok_or_else(|| {
            SyntaxCheckError::UnsupportedKind(invocation.kind.text.clone())
                .diagnostic(invocation.kind.span)
        })?;
    let mut values = vec![None; signature.startup_parameters.len()];
    let mut positional_count = 0usize;
    for argument in &invocation.arguments {
        match argument {
            Argument::Positional(expression) => {
                if positional_count >= signature.startup_parameters.len() {
                    return Err(SyntaxCheckError::TooManyPositional(signature.kind.clone())
                        .diagnostic(expression.span));
                }
                let parameter = &signature.startup_parameters[positional_count];
                values[positional_count] = Some(
                    resolver
                        .resolve_expression(
                            expression,
                            catalog.structured_type(&parameter.value_type),
                        )
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
                        .resolve_expression(
                            value,
                            catalog
                                .structured_type(&signature.startup_parameters[index].value_type),
                        )
                        .map_err(|error| error.diagnostic(value.span))?,
                );
            }
        }
    }
    let mut startup_bindings = Vec::with_capacity(signature.startup_parameters.len());
    for (index, parameter) in signature.startup_parameters.iter().enumerate() {
        let value =
            resolve_bound_value(index, &mut values, signature, catalog, &mut BTreeSet::new())
                .map_err(|error| error.diagnostic(invocation.span))?;
        startup_bindings.push(CheckedStartupBinding {
            name: parameter.name.clone(),
            value_type: parameter.value_type.clone(),
            value,
        });
    }
    Ok(CheckedCanonicalGear {
        name,
        kind: signature.kind.clone(),
        startup_parameters: catalog
            .canonical_startup_parameters(signature)
            .map_err(|_| SyntaxCheckDiagnostic {
                code: "CND-FRM-053",
                span: invocation.span,
                message: "startup parameter profile exceeds canonical bounds".into(),
            })?,
        startup_bindings,
        source_span: invocation.span,
    })
}

fn resolve_bound_value(
    index: usize,
    values: &mut [Option<CanonicalStartupValue>],
    signature: &KindSignature,
    catalog: &StartupCatalog,
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
            resolve_bound_value(reference, values, signature, catalog, visiting)?
        } else {
            CanonicalStartupValue::FormParameter(default.to_string())
        }
    } else if let Some(expected) = catalog.structured_type(&parameter.value_type) {
        let syntax = crate::structured_expression::parse(default, default, 0)
            .map_err(|(message, _)| SyntaxCheckError::StructuredExpression(message, None))?;
        let checked = crate::structured_startup::check_structured_expression(
            &syntax,
            expected,
            &mut |atomic, _| {
                if let Some(reference) = signature
                    .startup_parameters
                    .iter()
                    .position(|candidate| candidate.name == atomic.text)
                {
                    if values[reference].is_some()
                        || signature.startup_parameters[reference].default.is_some()
                    {
                        resolve_bound_value(reference, values, signature, catalog, visiting)
                            .map_err(|error| error.diagnostic(atomic.span))
                    } else {
                        Ok(CanonicalStartupValue::FormParameter(atomic.text.clone()))
                    }
                } else if is_atomic_literal(&atomic.text) {
                    Ok(CanonicalStartupValue::Literal(atomic.text.clone()))
                } else {
                    Err(SyntaxCheckError::UnsupportedExpression(atomic.text.clone())
                        .diagnostic(atomic.span))
                }
            },
        )
        .map_err(|diagnostic| SyntaxCheckError::StructuredExpression(diagnostic.message, None))?;
        if !checked.satisfies_concrete_bounds() {
            return Err(SyntaxCheckError::StructuredExpression(
                "structured default exceeds the finite canonical encoding bound".into(),
                None,
            ));
        }
        CanonicalStartupValue::Structured(checked)
    } else {
        CanonicalStartupValue::Literal(default.to_string())
    };
    visiting.remove(&index);
    values[index] = Some(value.clone());
    Ok(value)
}
