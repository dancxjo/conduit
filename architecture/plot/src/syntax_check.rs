use crate::checked_syntax::{
    CanonicalStartupValue, CheckedCanonicalCord, CheckedCanonicalGear, CheckedCanonicalPlot,
    CheckedCordStage, CheckedStartupBinding, CheckedStartupParameter, CheckedSyntaxDocument,
    KindSignature, SourceSugarExpansion, SourceSugarOperandBinding, StartupCatalog,
    StartupParameterSignature, SyntaxCheckDiagnostic, SyntaxCheckError,
};
use crate::prelude::*;
use crate::syntax::{
    Argument, BackStatement, CordStage, Invocation, MatchedRoute, MatchedRoutePattern, PlotSyntax,
    SyntaxDocument,
};
use crate::syntax_identity::{canonical_cord, canonical_gear, checked_identity};
use alloc::collections::{BTreeMap, BTreeSet};
use conduit_core::{
    CheckedFront, StructuredInfoTypeShape, StructuredSelector, UnmatchedVariantDisposition,
};

mod lexical_plots;
mod matched_route;
mod resolution;
mod shared_pool;
mod specialization;
mod structured_selector;
use resolution::{is_atomic_literal, Resolver};
use shared_pool::{check_pool_declarations, checked_pool};
use specialization::specialize_named_type_parameters;

pub(crate) fn is_local_plot_identity(name: &str) -> bool {
    lexical_plots::is_local_identity(name)
}

const STANDARD_GLYPH_BINDINGS: [(&str, &str); 5] = [
    ("><", "flow/merge"),
    ("&>", "flow/zip"),
    ("?>", "flow/race"),
    ("<>", "state/combine-latest"),
    ("@", "current/sample"),
];

pub(crate) struct PendingSourceSugarExpansion {
    plot: String,
    authored: String,
    source_span: crate::Span,
    ordinary_kind: String,
    input_ports: Vec<String>,
    output_ports: Vec<String>,
    operand_bindings: Vec<SourceSugarOperandBinding>,
    canonical_replacement: Option<String>,
}

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
    let glyph_notation_scope = crate::resolve_glyph_notation_scope(document, catalog)?;
    let glyph_notations = crate::glyph_notation::verify_declarations(document, catalog, None)?;
    // Literal elaboration supplies checked usage witnesses. The current ordinary
    // expression AST contains no admitted typed literal uses.
    glyph_notation_scope.require_used(&BTreeSet::new())?;
    let aliased_catalog = crate::native_type::install_import_aliases(document, catalog)?;
    let (native_types, checked_catalog) =
        crate::native_type::check_native_types(&document.types, &aliased_catalog)?;
    let type_forms =
        crate::type_form::check_type_forms(&document.type_forms, &document.types, &native_types)?;
    let catalog = &checked_catalog;
    let lexical_plots = lexical_plots::lower(&document.plots)?;
    let unresolved_plot_signatures = plot_signatures(&lexical_plots)?;
    let mut plot_fronts = BTreeMap::new();
    for plot in lexical_plots.iter().filter(|plot| {
        plot.front.type_parameters.is_empty() && plot.front.kind_parameters.is_empty()
    }) {
        plot_fronts.insert(
            plot.name.text.clone(),
            crate::value_type::checked_front(plot, catalog)?,
        );
    }
    let mut lexical_document = document.clone();
    lexical_document.plots = lexical_plots;
    let (plots, pending_source_sugar_expansions) = resolve_use_declarations(
        &lexical_document,
        catalog,
        &BTreeMap::new(),
        &unresolved_plot_signatures,
        &plot_fronts,
    )?;
    let plots = specialize_named_type_parameters(plots, catalog)?;
    let plot_signatures = plot_signatures(&plots)?;
    let mut plot_fronts = BTreeMap::new();
    for plot in &plots {
        plot_fronts.insert(
            plot.name.text.clone(),
            crate::value_type::checked_front(plot, catalog)?,
        );
    }
    let mut checked_plots = Vec::with_capacity(plots.len());
    for plot in &plots {
        checked_plots.push(check_plot(plot, catalog, &plot_signatures, &plot_fronts)?);
    }
    checked_plots.sort_by(|left, right| left.name.cmp(&right.name));
    let source_sugar_expansions = pending_source_sugar_expansions
        .into_iter()
        .chain(plots.iter().filter_map(expression_body_expansion))
        .map(|expansion| SourceSugarExpansion {
            checked_plot_id: checked_plots
                .iter()
                .find(|plot| plot.name == expansion.plot)
                .expect("source sugar names the Plot checked in this document")
                .checked_plot_id
                .clone(),
            plot: expansion.plot,
            authored: expansion.authored,
            source_span: expansion.source_span,
            ordinary_kind: expansion.ordinary_kind,
            input_ports: expansion.input_ports,
            output_ports: expansion.output_ports,
            operand_bindings: expansion.operand_bindings,
            canonical_replacement: expansion.canonical_replacement,
        })
        .collect();
    let mut structured_types =
        catalog
            .structured_types_by_value_kind()
            .map_err(|_| SyntaxCheckDiagnostic {
                code: "CND-FRM-053",
                span: crate::Span {
                    start: 0,
                    end: 0,
                    line: 1,
                    column: 1,
                    end_line: 1,
                    end_column: 1,
                },
                message: "structured type registry exceeds canonical bounds".into(),
            })?;
    for plot in &plots {
        for (source_type, optional) in plot
            .front
            .startup_parameters
            .iter()
            .map(|parameter| (parameter.value_type.text.as_str(), parameter.optional))
            .chain(plot.front.runtime_ports.iter().map(|port| {
                (
                    port.value_type.text.as_str(),
                    matches!(
                        port.temporal,
                        crate::RuntimePortTemporal::OptionalValue
                            | crate::RuntimePortTemporal::CurrentOptional
                    ),
                )
            }))
        {
            if optional {
                let value_type = crate::value_type::checked_optional_type(source_type, catalog)
                    .map_err(|_| SyntaxCheckDiagnostic {
                        code: "CND-FRM-053",
                        span: plot.span,
                        message: "optional type exceeds canonical finite bounds".into(),
                    })?;
                let value_kind = value_type
                    .profile()
                    .map_err(|_| SyntaxCheckDiagnostic {
                        code: "CND-FRM-053",
                        span: plot.span,
                        message: "optional type profile exceeds canonical finite bounds".into(),
                    })?
                    .value_kind()
                    .clone();
                structured_types.insert(value_kind, value_type);
            }
        }
    }
    for retained in checked_plots
        .iter()
        .flat_map(|plot| &plot.gears)
        .filter_map(|gear| gear.retained.as_deref())
    {
        let value_kind = retained
            .value_type
            .profile()
            .map_err(|_| SyntaxCheckDiagnostic {
                code: "CND-FRM-053",
                span: crate::Span {
                    start: 0,
                    end: 0,
                    line: 1,
                    column: 1,
                    end_line: 1,
                    end_column: 1,
                },
                message: "retained value profile exceeds canonical finite bounds".into(),
            })?
            .value_kind()
            .clone();
        structured_types.insert(value_kind, retained.value_type.clone());
    }
    Ok(CheckedSyntaxDocument {
        source_document_id: document.source_document_id(),
        native_types,
        retained_native_types: catalog.retained_native_types(),
        type_forms,
        glyph_notations,
        plots: checked_plots,
        source_sugar_expansions,
        structured_types,
        exact_initial_info: catalog.exact_initial_info.clone(),
    })
}

fn expression_body_expansion(plot: &PlotSyntax) -> Option<PendingSourceSugarExpansion> {
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
            body.expression.text,
            output.name.text
        )),
    })
}

pub(crate) fn resolve_use_declarations(
    document: &SyntaxDocument,
    catalog: &StartupCatalog,
    source_paths: &BTreeMap<String, String>,
    plots: &BTreeMap<String, KindSignature>,
    plot_fronts: &BTreeMap<String, CheckedFront>,
) -> Result<(Vec<PlotSyntax>, Vec<PendingSourceSugarExpansion>), SyntaxCheckDiagnostic> {
    let mut aliases = BTreeMap::<String, (String, crate::Span, bool)>::new();
    if document.standard_glyphs {
        let span = source_start_span();
        for (glyph, kind) in STANDARD_GLYPH_BINDINGS {
            aliases.insert(glyph.into(), (kind.into(), span, true));
        }
    }
    for declaration in &document.uses {
        let path = declaration.path.as_str();
        if catalog.structured_type(path).is_some() || catalog.native_families.contains_key(path) {
            continue;
        }
        let canonical = if catalog.get(path).is_some() || plots.contains_key(path) {
            path
        } else if let Some(plot) = source_paths.get(path) {
            plot
        } else {
            return Err(use_diagnostic(
                declaration.path_span,
                format!(
                    "with path '{path}' does not resolve to an installed Kind or checked source Plot"
                ),
            ));
        };
        if plots.contains_key(&declaration.alias.text) {
            return Err(use_diagnostic(
                declaration.alias.span,
                format!(
                    "with alias '{}' conflicts with a source Plot name",
                    declaration.alias.text
                ),
            ));
        }
        if document.standard_glyphs
            && STANDARD_GLYPH_BINDINGS
                .iter()
                .any(|(glyph, _)| *glyph == declaration.alias.text)
        {
            return Err(use_diagnostic(
                declaration.alias.span,
                format!(
                    "standard glyph '{}' is already in scope; put 'sans glyphs' before rebinding it",
                    declaration.alias.text
                ),
            ));
        }
        if aliases
            .insert(
                declaration.alias.text.clone(),
                (canonical.to_string(), declaration.alias.span, false),
            )
            .is_some()
        {
            return Err(use_diagnostic(
                declaration.alias.span,
                format!("duplicate with alias '{}'", declaration.alias.text),
            ));
        }
    }

    let mut resolved = document.plots.clone();
    let mut source_sugar_expansions = Vec::new();
    for plot in &mut resolved {
        resolve_plot_aliases(
            plot,
            &mut aliases,
            catalog,
            plot_fronts,
            &mut source_sugar_expansions,
        )?;
    }
    if let Some((alias, (_, span, _))) = aliases.iter().find(|(_, (_, _, used))| !*used) {
        return Err(use_diagnostic(
            *span,
            format!("unused with alias '{alias}'"),
        ));
    }
    Ok((resolved, source_sugar_expansions))
}

fn resolve_plot_aliases(
    plot: &mut PlotSyntax,
    aliases: &mut BTreeMap<String, (String, crate::Span, bool)>,
    catalog: &StartupCatalog,
    plot_fronts: &BTreeMap<String, CheckedFront>,
    source_sugar_expansions: &mut Vec<PendingSourceSugarExpansion>,
) -> Result<(), SyntaxCheckDiagnostic> {
    reject_alias_shadowing(plot, aliases)?;
    for statement in &mut plot.back {
        match statement {
            BackStatement::NamedGear(gear) => {
                resolve_invocation_alias(&mut gear.invocation, aliases)
            }
            BackStatement::Cord(cord) => resolve_stage_aliases(
                &plot.name.text,
                &mut cord.stages,
                aliases,
                catalog,
                plot_fronts,
                source_sugar_expansions,
            )?,
            BackStatement::MatchedRoute(route) => {
                for arm in &mut route.arms {
                    resolve_stage_aliases(
                        &plot.name.text,
                        &mut arm.stages,
                        aliases,
                        catalog,
                        plot_fronts,
                        source_sugar_expansions,
                    )?;
                }
            }
            BackStatement::Pool(pool) => {
                if let Some((canonical, _, used)) = aliases.get_mut(&pool.member_plot.text) {
                    pool.member_plot.text.clone_from(canonical);
                    *used = true;
                }
            }
            BackStatement::LocalValue(_) => {}
        }
    }
    for local in &mut plot.local_plots {
        resolve_plot_aliases(
            local,
            aliases,
            catalog,
            plot_fronts,
            source_sugar_expansions,
        )?;
    }
    Ok(())
}

fn source_start_span() -> crate::Span {
    crate::Span {
        start: 0,
        end: 0,
        line: 1,
        column: 1,
        end_line: 1,
        end_column: 1,
    }
}

fn resolve_stage_aliases(
    plot_name: &str,
    stages: &mut [CordStage],
    aliases: &mut BTreeMap<String, (String, crate::Span, bool)>,
    catalog: &StartupCatalog,
    plot_fronts: &BTreeMap<String, CheckedFront>,
    source_sugar_expansions: &mut Vec<PendingSourceSugarExpansion>,
) -> Result<(), SyntaxCheckDiagnostic> {
    for stage in stages {
        match stage {
            CordStage::InlineGear(invocation) => resolve_invocation_alias(invocation, aliases),
            CordStage::Reference(reference) => {
                if let Some((canonical, _, used)) = aliases.get_mut(&reference.text) {
                    let span = reference.span;
                    *stage = CordStage::InlineGear(Invocation {
                        kind: crate::syntax::SpannedText {
                            text: canonical.clone(),
                            span,
                        },
                        arguments: Vec::new(),
                        span,
                    });
                    *used = true;
                }
            }
            CordStage::Glyph(glyph) => {
                if let Some((canonical, _, used)) = aliases.get_mut(&glyph.text) {
                    let fore = if let Some(fore) = plot_fronts.get(canonical) {
                        fore.clone()
                    } else {
                        match catalog.fore_for_arity(canonical, 1) {
                            Ok(Some(fore)) => fore,
                            Ok(None) => {
                                return Err(use_diagnostic(
                                    glyph.span,
                                    format!(
                                        "glyph '{}' requires the exact checked Fore for '{}'",
                                        glyph.text, canonical
                                    ),
                                ));
                            }
                            Err(message) => return Err(use_diagnostic(glyph.span, message)),
                        }
                    };
                    if fore.shorthand().is_none() {
                        return Err(use_diagnostic(
                            glyph.span,
                            format!(
                                "unary glyph '{}' requires exactly one shorthand input and output",
                                glyph.text
                            ),
                        ));
                    }
                    let (input_port, output_port) =
                        fore.shorthand().expect("shorthand presence checked above");
                    let span = glyph.span;
                    source_sugar_expansions.push(PendingSourceSugarExpansion {
                        plot: plot_name.into(),
                        authored: glyph.text.clone(),
                        source_span: span,
                        ordinary_kind: canonical.clone(),
                        input_ports: vec![input_port.as_str().into()],
                        output_ports: vec![output_port.as_str().into()],
                        operand_bindings: Vec::new(),
                        canonical_replacement: Some(canonical.clone()),
                    });
                    *stage = CordStage::InlineGear(Invocation {
                        kind: crate::syntax::SpannedText {
                            text: canonical.clone(),
                            span,
                        },
                        arguments: Vec::new(),
                        span,
                    });
                    *used = true;
                } else {
                    *stage = CordStage::Reference(glyph.clone());
                }
            }
            CordStage::RelationalGlyph {
                operands,
                glyph,
                span,
            } => {
                let Some((canonical, _, used)) = aliases.get_mut(&glyph.text) else {
                    return Err(use_diagnostic(
                        glyph.span,
                        format!("glyph '{}' did not resolve in lexical scope", glyph.text),
                    ));
                };
                let fore = if let Some(fore) = plot_fronts.get(canonical) {
                    fore.clone()
                } else {
                    match catalog.fore_for_arity(canonical, operands.len()) {
                        Ok(Some(fore)) => fore,
                        Ok(None) => catalog.fore(canonical).cloned().ok_or_else(|| {
                            use_diagnostic(
                                glyph.span,
                                format!(
                                    "glyph '{}' requires the exact checked Fore for '{}'",
                                    glyph.text, canonical
                                ),
                            )
                        })?,
                        Err(message) => return Err(use_diagnostic(glyph.span, message)),
                    }
                };
                if fore.inputs().len() != operands.len() || fore.outputs().len() != 1 {
                    return Err(use_diagnostic(
                        glyph.span,
                        format!(
                            "relational glyph '{}' supplies {} operands but '{}' has {} inputs and {} outputs",
                            glyph.text,
                            operands.len(),
                            canonical,
                            fore.inputs().len(),
                            fore.outputs().len()
                        ),
                    ));
                }
                let invocation = Invocation {
                    kind: crate::syntax::SpannedText {
                        text: canonical.clone(),
                        span: glyph.span,
                    },
                    arguments: Vec::new(),
                    span: glyph.span,
                };
                let input_ports: Vec<String> = fore
                    .inputs()
                    .iter()
                    .map(|port| port.port_id.as_str().to_string())
                    .collect();
                let output_port = fore.outputs()[0].port_id.as_str().to_string();
                source_sugar_expansions.push(PendingSourceSugarExpansion {
                    plot: plot_name.into(),
                    authored: glyph.text.clone(),
                    source_span: glyph.span,
                    ordinary_kind: canonical.clone(),
                    input_ports: input_ports.clone(),
                    output_ports: vec![output_port.clone()],
                    operand_bindings: operands
                        .iter()
                        .zip(&input_ports)
                        .map(|(source, input_port)| SourceSugarOperandBinding {
                            source: source.text.clone(),
                            input_port: input_port.clone(),
                        })
                        .collect(),
                    canonical_replacement: None,
                });
                *used = true;
                *stage = CordStage::RelationalGear {
                    operands: operands.clone(),
                    invocation,
                    input_ports,
                    output_port,
                    span: *span,
                };
            }
            CordStage::RelationalGear { .. } => {}
            CordStage::TerminalProjection { .. }
            | CordStage::Cancellation { .. }
            | CordStage::When(_)
            | CordStage::Literal(_)
            | CordStage::PureExpression(_)
            | CordStage::StructuredSelector(_) => {}
        }
    }
    Ok(())
}

fn resolve_invocation_alias(
    invocation: &mut Invocation,
    aliases: &mut BTreeMap<String, (String, crate::Span, bool)>,
) {
    if let Some((canonical, _, used)) = aliases.get_mut(&invocation.kind.text) {
        invocation.kind.text.clone_from(canonical);
        *used = true;
    }
}

fn reject_alias_shadowing(
    plot: &PlotSyntax,
    aliases: &BTreeMap<String, (String, crate::Span, bool)>,
) -> Result<(), SyntaxCheckDiagnostic> {
    for name in plot
        .front
        .type_parameters
        .iter()
        .map(|parameter| (&parameter.name.text, parameter.name.span))
        .chain(
            plot.front
                .kind_parameters
                .iter()
                .map(|parameter| (&parameter.name.text, parameter.name.span)),
        )
        .chain(
            plot.front
                .startup_parameters
                .iter()
                .map(|parameter| (&parameter.name.text, parameter.name.span)),
        )
        .chain(
            plot.front
                .runtime_ports
                .iter()
                .map(|port| (&port.name.text, port.name.span)),
        )
        .chain(plot.back.iter().filter_map(|statement| match statement {
            BackStatement::NamedGear(gear) => Some((&gear.name.text, gear.name.span)),
            BackStatement::Pool(pool) => Some((&pool.name.text, pool.name.span)),
            BackStatement::LocalValue(local) => Some((&local.name.text, local.name.span)),
            BackStatement::Cord(_) | BackStatement::MatchedRoute(_) => None,
        }))
        .chain(
            plot.local_plots
                .iter()
                .map(|local| (&local.name.text, local.name.span)),
        )
    {
        if aliases.contains_key(name.0) {
            return Err(use_diagnostic(
                name.1,
                format!("binding '{}' shadows a with alias", name.0),
            ));
        }
    }
    Ok(())
}

fn use_diagnostic(span: crate::Span, message: String) -> SyntaxCheckDiagnostic {
    SyntaxCheckDiagnostic {
        code: "CND-FRM-056",
        span,
        message,
    }
}

pub(crate) fn plot_signatures(
    plots: &[PlotSyntax],
) -> Result<BTreeMap<String, KindSignature>, SyntaxCheckDiagnostic> {
    let mut signatures = BTreeMap::new();
    for plot in plots {
        if signatures.contains_key(&plot.name.text) {
            return Err(SyntaxCheckError::DuplicateImmutable(plot.name.text.clone())
                .diagnostic(plot.name.span));
        }
        let mut names = BTreeSet::new();
        let mut type_names = BTreeSet::new();
        for parameter in &plot.front.type_parameters {
            if !names.insert(parameter.name.text.clone()) {
                return Err(
                    SyntaxCheckError::DuplicateImmutable(parameter.name.text.clone())
                        .diagnostic(parameter.span),
                );
            }
            type_names.insert(parameter.name.text.clone());
        }
        for parameter in &plot.front.kind_parameters {
            if !names.insert(parameter.name.text.clone()) {
                return Err(
                    SyntaxCheckError::DuplicateImmutable(parameter.name.text.clone())
                        .diagnostic(parameter.span),
                );
            }
            type_names.insert(parameter.name.text.clone());
        }
        let mut startup_parameters = Vec::new();
        for parameter in &plot.front.startup_parameters {
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
        if let Some(port) = plot
            .front
            .runtime_ports
            .iter()
            .find(|port| type_names.contains(&port.name.text))
        {
            return Err(
                SyntaxCheckError::AmbiguousFrontName(port.name.text.clone()).diagnostic(port.span)
            );
        }
        signatures.insert(
            plot.name.text.clone(),
            KindSignature {
                kind: plot.name.text.clone(),
                startup_parameters,
            },
        );
    }
    Ok(signatures)
}

fn check_plot(
    plot: &PlotSyntax,
    catalog: &StartupCatalog,
    plot_signatures: &BTreeMap<String, KindSignature>,
    plot_fronts: &BTreeMap<String, CheckedFront>,
) -> Result<CheckedCanonicalPlot, SyntaxCheckDiagnostic> {
    let signature = plot_signatures
        .get(&plot.name.text)
        .expect("every parsed plot has a derived signature");
    let parameters = checked_parameters(signature, catalog, plot)?;
    let parameter_names = signature
        .startup_parameters
        .iter()
        .map(|parameter| parameter.name.clone())
        .collect::<BTreeSet<_>>();
    let mut runtime_names = BTreeSet::new();
    for port in &plot.front.runtime_ports {
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
    let pool_names = check_pool_declarations(plot, &front_names, plot_fronts)?;
    for statement in &plot.back {
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
    for statement in &plot.back {
        match statement {
            BackStatement::NamedGear(gear) => {
                let mut checked = check_invocation(
                    Some(gear.name.text.clone()),
                    &gear.invocation,
                    catalog,
                    plot_signatures,
                    Some(plot_fronts),
                    gear.activation.clone(),
                    &mut resolver,
                )?;
                checked.retained = gear
                    .retained
                    .as_deref()
                    .map(|retained| {
                        let value_type = if retained.optional {
                            crate::value_type::checked_optional_type(
                                &retained.value_type.text,
                                catalog,
                            )
                        } else {
                            crate::value_type::checked_value_type(
                                &retained.value_type.text,
                                catalog,
                            )
                        }
                        .map_err(|_| SyntaxCheckDiagnostic {
                            code: "CND-FRM-053",
                            span: retained.value_type.span,
                            message: "retained value type exceeds canonical finite bounds".into(),
                        })?;
                        let value_kind = match value_type.shape() {
                            conduit_core::StructuredInfoTypeShape::Leaf(kind) => kind.clone(),
                            _ => value_type
                                .profile()
                                .map_err(|_| SyntaxCheckDiagnostic {
                                    code: "CND-FRM-053",
                                    span: retained.value_type.span,
                                    message: "retained value type has no exact checked identity"
                                        .into(),
                                })?
                                .value_kind()
                                .clone(),
                        };
                        // `T?(value)` is the canonical KEEP initializer sugar for
                        // `some(T)`. The finite optional variant itself remains the
                        // checked State type; the authored initializer is checked
                        // against its payload type and wrapped during expansion.
                        // With no initializer expansion supplies canonical `none`.
                        let initial_type = if retained.optional {
                            crate::value_type::checked_value_type(
                                &retained.value_type.text,
                                catalog,
                            )
                            .map_err(|_| SyntaxCheckDiagnostic {
                                code: "CND-FRM-053",
                                span: retained.value_type.span,
                                message:
                                    "retained optional payload exceeds canonical finite bounds"
                                        .into(),
                            })?
                        } else {
                            value_type.clone()
                        };
                        let initial = retained
                            .initial
                            .as_ref()
                            .map(|initial| {
                                resolver
                                    .resolve_expression(initial, Some(&initial_type))
                                    .map_err(|error| error.diagnostic(initial.span))
                            })
                            .transpose()?;
                        if let Some(CanonicalStartupValue::Structured(value)) = &initial {
                            crate::native_type::validate_concrete_value(
                                &retained.value_type.text,
                                value,
                                catalog,
                                retained.value_type.span,
                            )?;
                        }
                        Ok(Box::new(crate::CheckedRetainedValue {
                            value_type,
                            value_kind,
                            optional: retained.optional,
                            maximum_bytes: retained.maximum_bytes,
                            initial,
                            duration: retained.duration,
                        }))
                    })
                    .transpose()?;
                gears.push(checked);
            }
            BackStatement::Cord(cord) => {
                check_direct_cord_type(cord, plot, catalog)?;
                let stages = check_cord_stages(
                    &cord.stages,
                    catalog,
                    plot_signatures,
                    &mut resolver,
                    &mut gears,
                )?;
                cords.push(CheckedCanonicalCord { stages });
            }
            BackStatement::MatchedRoute(route) => cords.extend(check_matched_route(
                route,
                catalog,
                plot_signatures,
                &mut resolver,
                &mut gears,
            )?),
            BackStatement::Pool(pool) => pools.push(checked_pool(pool, plot_fronts)),
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
    let runtime_front = plot_fronts
        .get(&plot.name.text)
        .expect("every parsed plot has a checked front")
        .clone();
    let checked_plot_id = checked_identity(
        (&plot.name.text, plot.completion),
        crate::syntax_identity::CheckedIdentityFront {
            parameters: &parameters,
            runtime_ports: &plot.front.runtime_ports,
            runtime_front: &runtime_front,
            shorthand: plot.front.shorthand.as_ref().map(|pair| {
                (
                    pair.input_port.text.as_str(),
                    pair.output_port.text.as_str(),
                )
            }),
        },
        &gears,
        &cords,
        &pools,
    );
    Ok(CheckedCanonicalPlot {
        checked_plot_id,
        name: plot.name.text.clone(),
        completion: plot.completion,
        startup_parameters: parameters,
        runtime_ports: plot.front.runtime_ports.clone(),
        runtime_front,
        shorthand: plot
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

fn check_direct_cord_type(
    cord: &crate::Cord,
    plot: &PlotSyntax,
    catalog: &StartupCatalog,
) -> Result<(), SyntaxCheckDiagnostic> {
    let [CordStage::Reference(source), CordStage::Reference(target)] = cord.stages.as_slice()
    else {
        return Ok(());
    };
    let Some(source_port) = plot
        .front
        .runtime_ports
        .iter()
        .find(|port| port.name.text == source.text)
    else {
        return Ok(());
    };
    let Some(target_port) = plot
        .front
        .runtime_ports
        .iter()
        .find(|port| port.name.text == target.text)
    else {
        return Ok(());
    };
    let source_kind = crate::value_type::checked_value_kind_with_modality(
        &source_port.value_type.text,
        matches!(
            source_port.temporal,
            crate::RuntimePortTemporal::OptionalValue | crate::RuntimePortTemporal::CurrentOptional
        ),
        catalog,
    )
    .map_err(|_| type_mismatch(cord.span, "source has no exact checked semantic Type"))?;
    let target_kind = crate::value_type::checked_value_kind_with_modality(
        &target_port.value_type.text,
        matches!(
            target_port.temporal,
            crate::RuntimePortTemporal::OptionalValue | crate::RuntimePortTemporal::CurrentOptional
        ),
        catalog,
    )
    .map_err(|_| type_mismatch(cord.span, "target has no exact checked semantic Type"))?;
    if source_kind != target_kind {
        return Err(type_mismatch(
            cord.span,
            &alloc::format!(
                "semantic Type mismatch: '{}' and '{}' are not interchangeable even when their Forms are compatible",
                source_port.value_type.text,
                target_port.value_type.text
            ),
        ));
    }
    Ok(())
}

fn type_mismatch(span: crate::Span, message: &str) -> SyntaxCheckDiagnostic {
    SyntaxCheckDiagnostic {
        code: "CND-FRM-058",
        span,
        message: message.into(),
    }
}

fn check_cord_stages(
    source: &[CordStage],
    catalog: &StartupCatalog,
    plot_signatures: &BTreeMap<String, KindSignature>,
    resolver: &mut Resolver<'_>,
    gears: &mut Vec<CheckedCanonicalGear>,
) -> Result<Vec<CheckedCordStage>, SyntaxCheckDiagnostic> {
    let mut stages = Vec::with_capacity(source.len());
    for stage in source {
        match stage {
            CordStage::Reference(reference) => {
                stages.push(CheckedCordStage::Reference(reference.text.clone()));
            }
            CordStage::Glyph(glyph) => {
                return Err(use_diagnostic(
                    glyph.span,
                    format!("glyph '{}' did not resolve in lexical scope", glyph.text),
                ));
            }
            CordStage::RelationalGlyph { glyph, .. } => {
                return Err(use_diagnostic(
                    glyph.span,
                    format!("glyph '{}' did not resolve in lexical scope", glyph.text),
                ));
            }
            CordStage::RelationalGear {
                operands,
                invocation,
                input_ports,
                output_port,
                ..
            } => {
                let gear = check_invocation(
                    None,
                    invocation,
                    catalog,
                    plot_signatures,
                    None,
                    None,
                    resolver,
                )?;
                stages.push(CheckedCordStage::RelationalGear {
                    operands: operands
                        .iter()
                        .map(|operand| operand.text.clone())
                        .collect(),
                    gear: gear.clone(),
                    input_ports: input_ports.clone(),
                    output_port: output_port.clone(),
                });
                gears.push(gear);
            }
            CordStage::TerminalProjection {
                endpoint,
                terminal,
                span,
            } => stages.push(CheckedCordStage::TerminalProjection {
                endpoint: endpoint.text.clone(),
                terminal: *terminal,
                source_span: *span,
            }),
            CordStage::Cancellation { gear, span } => {
                stages.push(CheckedCordStage::Cancellation {
                    gear: gear.text.clone(),
                    source_span: *span,
                });
            }
            CordStage::When(expression) => stages.push(CheckedCordStage::When {
                expression: expression.syntax.clone(),
                source_span: expression.span,
            }),
            CordStage::InlineGear(invocation) => {
                let gear = check_invocation(
                    None,
                    invocation,
                    catalog,
                    plot_signatures,
                    None,
                    None,
                    resolver,
                )?;
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
            CordStage::PureExpression(expression) => {
                stages.push(CheckedCordStage::PureExpression {
                    expression: expression.syntax.clone(),
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
    plot_signatures: &BTreeMap<String, KindSignature>,
    resolver: &mut Resolver<'_>,
    gears: &mut Vec<CheckedCanonicalGear>,
) -> Result<Vec<CheckedCanonicalCord>, SyntaxCheckDiagnostic> {
    if route
        .arms
        .iter()
        .any(|arm| matches!(arm.pattern, MatchedRoutePattern::Guard { .. }))
    {
        return matched_route::check_guarded(route, catalog, plot_signatures, resolver, gears);
    }
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
            MatchedRoutePattern::Guard { span, .. } => {
                return Err(route_diagnostic(
                    *span,
                    "variant and guarded tracks cannot be mixed",
                ));
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
        cords.push(checked_route_track(
            route,
            arm,
            selector,
            *span,
            (catalog, plot_signatures, resolver, gears),
        )?);
    }
    Ok(cords)
}

fn checked_route_track(
    route: &MatchedRoute,
    arm: &crate::MatchedRouteArm,
    selector: StructuredSelector,
    selector_span: crate::Span,
    context: (
        &StartupCatalog,
        &BTreeMap<String, KindSignature>,
        &mut Resolver<'_>,
        &mut Vec<CheckedCanonicalGear>,
    ),
) -> Result<CheckedCanonicalCord, SyntaxCheckDiagnostic> {
    let (catalog, plot_signatures, resolver, gears) = context;
    let mut stages = vec![
        CheckedCordStage::Reference(route.source.text.clone()),
        CheckedCordStage::StructuredSelector {
            selector,
            source_span: selector_span,
        },
    ];
    let carried = arm.stages.first().is_some_and(
        |stage| matches!(stage, CordStage::Reference(reference) if reference.text == "_"),
    );
    if let Some(CordStage::Reference(reference)) = arm
        .stages
        .iter()
        .skip(usize::from(carried))
        .find(|stage| matches!(stage, CordStage::Reference(reference) if reference.text == "_"))
    {
        return Err(route_diagnostic(
            reference.span,
            "carried value '_' must be the first stage of a matched track",
        ));
    }
    stages.extend(check_cord_stages(
        &arm.stages[usize::from(carried)..],
        catalog,
        plot_signatures,
        resolver,
        gears,
    )?);
    Ok(CheckedCanonicalCord { stages })
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
    plot: &PlotSyntax,
) -> Result<Vec<CheckedStartupParameter>, SyntaxCheckDiagnostic> {
    let mut values = vec![None; signature.startup_parameters.len()];
    let mut checked = Vec::with_capacity(signature.startup_parameters.len());
    for (index, parameter) in signature.startup_parameters.iter().enumerate() {
        let default = if parameter.default.is_some() {
            let value =
                resolve_bound_value(index, &mut values, signature, catalog, &mut BTreeSet::new())
                    .map_err(|error| {
                    let parameter = &plot.front.startup_parameters[index];
                    error.diagnostic(
                        parameter
                            .default
                            .as_ref()
                            .map_or(parameter.span, |value| value.span),
                    )
                })?;
            Some(
                canonicalize_integer_value(value, &parameter.value_type, catalog)
                    .map_err(|error| error.diagnostic(plot.front.startup_parameters[index].span))?,
            )
        } else {
            None
        };
        if let Some(value) = &default {
            validate_quantity_type(
                value,
                &parameter.value_type,
                plot.front.startup_parameters[index].span,
            )?;
        }
        checked.push(CheckedStartupParameter {
            name: parameter.name.clone(),
            value_type: parameter.value_type.clone(),
            default,
            optional: plot.front.startup_parameters[index].optional,
            maximum_bytes: plot.front.startup_parameters[index].maximum_bytes,
        });
    }
    Ok(checked)
}

fn check_invocation(
    name: Option<String>,
    invocation: &Invocation,
    catalog: &StartupCatalog,
    plot_signatures: &BTreeMap<String, KindSignature>,
    plot_fronts: Option<&BTreeMap<String, CheckedFront>>,
    activation: Option<crate::ActivationSyntax>,
    resolver: &mut Resolver<'_>,
) -> Result<CheckedCanonicalGear, SyntaxCheckDiagnostic> {
    let signature = plot_signatures
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
                            crate::quantity_literal::selected_profile(
                                catalog,
                                &parameter.value_type,
                            )
                            .as_deref(),
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
                            crate::quantity_literal::selected_profile(
                                catalog,
                                &signature.startup_parameters[index].value_type,
                            )
                            .as_deref(),
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
        let value = canonicalize_integer_value(value, &parameter.value_type, catalog)
            .map_err(|error| error.diagnostic(invocation.span))?;
        validate_quantity_type(&value, &parameter.value_type, invocation.span)?;
        startup_bindings.push(CheckedStartupBinding {
            name: parameter.name.clone(),
            value_type: parameter.value_type.clone(),
            value,
        });
    }
    let activation = activation
        .map(|mode| {
            checked_activation(
                invocation,
                mode,
                plot_fronts.expect("activation checking receives source Plot fronts"),
                catalog,
                resolver,
            )
        })
        .transpose()?;
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
        retained: None,
        activation,
        source_span: invocation.span,
    })
}

fn checked_activation(
    invocation: &Invocation,
    mode: crate::ActivationSyntax,
    plot_fronts: &BTreeMap<String, CheckedFront>,
    catalog: &StartupCatalog,
    resolver: &mut Resolver<'_>,
) -> Result<crate::CheckedActivation, SyntaxCheckDiagnostic> {
    let front = plot_fronts
        .get(&invocation.kind.text)
        .ok_or_else(|| SyntaxCheckDiagnostic {
            code: "CND-FRM-062",
            span: invocation.span,
            message: "activate requires one exact checked source Plot".into(),
        })?;
    let contract = |location: conduit_core::FrontValueLocation| {
        front
            .value_contracts()
            .iter()
            .find(|value| value.location == location)
            .map(|value| value.contract.clone())
    };
    if let crate::ActivationSyntax::Fold { initial, .. }
    | crate::ActivationSyntax::Scan { initial, .. } = &mode
    {
        let (name, code) = if matches!(mode, crate::ActivationSyntax::Fold { .. }) {
            ("fold", "CND-FRM-064")
        } else {
            ("scan", "CND-FRM-065")
        };
        let accumulator = front
            .inputs()
            .iter()
            .find(|port| port.port_id.as_str() == "accumulator");
        let item = front
            .inputs()
            .iter()
            .find(|port| port.port_id.as_str() == "item");
        let combined = front
            .outputs()
            .iter()
            .find(|port| port.port_id.as_str() == "combined");
        let (Some(accumulator), Some(item), Some(combined)) = (accumulator, item, combined) else {
            return Err(SyntaxCheckDiagnostic {
                code,
                span: invocation.span,
                message: format!("{name} requires exact Value inputs 'accumulator' and 'item' and Value output 'combined'"),
            });
        };
        if front.inputs().len() != 2
            || front.outputs().len() != 1
            || accumulator.temporal != conduit_core::PortTemporal::Value
            || item.temporal != conduit_core::PortTemporal::Value
            || combined.temporal != conduit_core::PortTemporal::Value
            || accumulator.value_kind != combined.value_kind
            || accumulator.abnormal_kind != item.abnormal_kind
            || accumulator.abnormal_kind != combined.abnormal_kind
            || !front.startup_parameters().is_empty()
        {
            return Err(SyntaxCheckDiagnostic {
                code,
                span: invocation.span,
                message: format!("{name} combine must be startup-free and preserve one exact accumulator Value and abnormal terminal"),
            });
        }
        let initial = resolver
            .resolve_expression(
                initial,
                crate::quantity_literal::selected_profile(catalog, accumulator.value_kind.as_str())
                    .as_deref(),
            )
            .and_then(|value| {
                canonicalize_integer_value(value, accumulator.value_kind.as_str(), catalog)
            })
            .map_err(|error| error.diagnostic(initial.span))?;
        let accumulator_contract = contract(conduit_core::FrontValueLocation::Input(
            accumulator.port_id.clone(),
        ))
        .ok_or_else(|| SyntaxCheckDiagnostic {
            code,
            span: invocation.span,
            message: format!("{name} accumulator requires one exact finite value contract"),
        })?;
        let initial_accumulator_bytes = if let CanonicalStartupValue::Literal(literal) = &initial {
            if conduit_core::primitive_info_kind(accumulator.value_kind.as_str()).is_none() {
                let bytes = catalog
                    .exact_initial_info
                    .get(&(accumulator.value_kind.clone(), literal.clone()))
                    .ok_or_else(|| SyntaxCheckDiagnostic {
                        code: "CND-FRM-064",
                        span: invocation.span,
                        message: format!(
                            "{name} initial custom Info has no owner-validated exact Form"
                        ),
                    })?
                    .clone();
                accumulator_contract
                    .validate(&bytes)
                    .map_err(|_| SyntaxCheckDiagnostic {
                        code: "CND-FRM-064",
                        span: invocation.span,
                        message: format!(
                            "{name} initial Form differs from its exact accumulator Value contract"
                        ),
                    })?;
                Some(bytes)
            } else {
                None
            }
        } else {
            None
        };
        return Ok(crate::CheckedActivation {
            mode,
            selected_plot: invocation.kind.text.clone(),
            input: item.clone(),
            accumulator_input: Some(accumulator.clone()),
            output: combined.clone(),
            initial_accumulator: Some(initial),
            initial_accumulator_bytes,
            input_contract: contract(conduit_core::FrontValueLocation::Input(
                item.port_id.clone(),
            ))
            .ok_or_else(|| SyntaxCheckDiagnostic {
                code,
                span: invocation.span,
                message: format!("{name} item requires one exact finite value contract"),
            })?,
            output_contract: contract(conduit_core::FrontValueLocation::Output(
                combined.port_id.clone(),
            ))
            .ok_or_else(|| SyntaxCheckDiagnostic {
                code,
                span: invocation.span,
                message: format!("{name} output requires one exact finite value contract"),
            })?,
            abnormal_contract: item.abnormal_kind.as_ref().and_then(|_| {
                contract(conduit_core::FrontValueLocation::InputAbnormal(
                    item.port_id.clone(),
                ))
            }),
            accumulator_contract: Some(accumulator_contract),
        });
    }
    let ([input], [output]) = (front.inputs(), front.outputs()) else {
        return Err(SyntaxCheckDiagnostic {
            code: "CND-FRM-062",
            span: invocation.span,
            message: "activate requires an exact unary Value-to-Value Plot".into(),
        });
    };
    if input.temporal != conduit_core::PortTemporal::Value
        || output.temporal != conduit_core::PortTemporal::Value
        || input.abnormal_kind != output.abnormal_kind
        || !front.startup_parameters().is_empty()
    {
        return Err(SyntaxCheckDiagnostic {
            code: "CND-FRM-062",
            span: invocation.span,
            message: "activate requires startup-free Value fronts with one exact propagated abnormal terminal".into(),
        });
    }
    if matches!(mode, crate::ActivationSyntax::Select { .. })
        && output.value_kind.as_str() != conduit_core::BOOL_INFO_ID
    {
        return Err(SyntaxCheckDiagnostic {
            code: "CND-FRM-063",
            span: invocation.span,
            message: "select requires an exact Value-to-Boolean predicate; truthiness coercion is not permitted".into(),
        });
    }
    Ok(crate::CheckedActivation {
        mode,
        selected_plot: invocation.kind.text.clone(),
        input: input.clone(),
        accumulator_input: None,
        output: output.clone(),
        initial_accumulator: None,
        initial_accumulator_bytes: None,
        input_contract: contract(conduit_core::FrontValueLocation::Input(
            input.port_id.clone(),
        ))
        .ok_or_else(|| SyntaxCheckDiagnostic {
            code: "CND-FRM-062",
            span: invocation.span,
            message: "activation input requires one exact finite value contract".into(),
        })?,
        output_contract: contract(conduit_core::FrontValueLocation::Output(
            output.port_id.clone(),
        ))
        .ok_or_else(|| SyntaxCheckDiagnostic {
            code: "CND-FRM-062",
            span: invocation.span,
            message: "activation output requires one exact finite value contract".into(),
        })?,
        abnormal_contract: input.abnormal_kind.as_ref().and_then(|_| {
            contract(conduit_core::FrontValueLocation::InputAbnormal(
                input.port_id.clone(),
            ))
        }),
        accumulator_contract: None,
    })
}

fn canonicalize_integer_value(
    value: CanonicalStartupValue,
    source_type: &str,
    catalog: &StartupCatalog,
) -> Result<CanonicalStartupValue, SyntaxCheckError> {
    let CanonicalStartupValue::Literal(literal) = value else {
        return Ok(value);
    };
    let value_kind = crate::value_type::checked_value_kind(source_type, catalog).map_err(|_| {
        SyntaxCheckError::InvalidIntegerLiteral(format!(
            "startup type '{source_type}' has no exact checked identity"
        ))
    })?;
    match crate::integer_literal::canonicalize(&literal, value_kind.as_str()) {
        Ok(Some(canonical)) => Ok(CanonicalStartupValue::Literal(canonical)),
        Ok(None) => Ok(CanonicalStartupValue::Literal(literal)),
        Err(detail) => Err(SyntaxCheckError::InvalidIntegerLiteral(detail)),
    }
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
            CanonicalStartupValue::PlotParameter(default.to_string())
        }
    } else if let Some(expected) =
        crate::quantity_literal::selected_profile(catalog, &parameter.value_type).as_deref()
    {
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
                        Ok(CanonicalStartupValue::PlotParameter(atomic.text.clone()))
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
        match crate::quantity_literal::startup_quantity(default)? {
            Some(value) => CanonicalStartupValue::Quantity(value),
            None => CanonicalStartupValue::Literal(default.to_string()),
        }
    };
    visiting.remove(&index);
    values[index] = Some(value.clone());
    Ok(value)
}

fn validate_quantity_type(
    value: &CanonicalStartupValue,
    source_type: &str,
    span: crate::Span,
) -> Result<(), SyntaxCheckDiagnostic> {
    let CanonicalStartupValue::Quantity(quantity) = value else {
        return Ok(());
    };
    let kind = crate::value_type::canonical_value_kind(source_type);
    if kind.as_str() == conduit_core::QUANTITY_INFO_ID
        || conduit_core::validate_primitive_info(kind.as_str(), &quantity.encode()).is_ok()
    {
        return Ok(());
    }
    Err(SyntaxCheckDiagnostic {
        code: "CND-FRM-055",
        span,
        message: format!(
            "quantity unit '{}' has dimension {:?}, which cannot satisfy '{}'",
            quantity.unit().plot_suffix(),
            quantity.dimension(),
            source_type
        ),
    })
}
