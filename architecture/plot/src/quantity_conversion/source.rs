//! Preparation diagnostics mapped to exact quoted source, including local aliases.
use super::{contract, prepare_configuration, QuantityConversionPreparationRefusal, KIND};
use crate::{
    prelude::*, Argument, BackStatement, CanonicalStartupValue, CheckedSyntaxDocument, CordStage,
    ExpressionSyntax, Invocation, PlotSyntax, QuotedTextSourceMap, Span, SyntaxDocument,
};
use conduit_core::{ConfigurationEntry, ExactQuantityConversionRequestRefusal, SourceDocumentId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuantityConversionSourceDiagnostic {
    pub source_document_id: SourceDocumentId,
    pub span: Span,
    pub refusal: Box<QuantityConversionPreparationRefusal>,
}

/// Parameterized requests remain unresolved until concrete preparation. A
/// foreign checked Source cannot authorize correlation with a quoted token.
pub fn validate_source(
    syntax: &SyntaxDocument,
    checked: &CheckedSyntaxDocument,
) -> Result<(), QuantityConversionSourceDiagnostic> {
    let mut invocations = Vec::new();
    for plot in &syntax.plots {
        collect(plot, &mut invocations);
    }
    let fields = contract().configuration;
    for plot in &checked.plots {
        for gear in &plot.gears {
            if gear.kind != KIND {
                continue;
            }
            let diagnostic = |refusal, span| QuantityConversionSourceDiagnostic {
                source_document_id: checked.source_document_id.clone(),
                span,
                refusal: Box::new(refusal),
            };
            if syntax.source_document_id() != checked.source_document_id {
                return Err(diagnostic(
                    QuantityConversionPreparationRefusal::SourceCorrelation,
                    gear.source_span,
                ));
            }
            if gear
                .startup_bindings
                .iter()
                .any(|binding| matches!(binding.value, CanonicalStartupValue::PlotParameter(_)))
            {
                continue;
            }
            let invocation = invocations
                .iter()
                .copied()
                .find(|(invocation, _)| invocation.span == gear.source_span);
            let mut configuration = Vec::new();
            for binding in &gear.startup_bindings {
                let field = fields
                    .iter()
                    .find(|field| field.key == binding.name)
                    .ok_or_else(|| {
                        diagnostic(
                            QuantityConversionPreparationRefusal::Configuration,
                            gear.source_span,
                        )
                    })?;
                let value = crate::validate_startup_configuration(field, binding.value.clone())
                    .map_err(|_| {
                        diagnostic(
                            QuantityConversionPreparationRefusal::Configuration,
                            invocation
                                .and_then(|(call, plot)| {
                                    located(syntax.round_trip(), call, plot, &binding.name)
                                })
                                .unwrap_or(gear.source_span),
                        )
                    })?;
                configuration.push(ConfigurationEntry {
                    key: binding.name.clone(),
                    value,
                });
            }
            if let Err(refusal) = prepare_configuration(&configuration) {
                let field = match refusal {
                    QuantityConversionPreparationRefusal::Request(
                        ExactQuantityConversionRequestRefusal::Target(_)
                        | ExactQuantityConversionRequestRefusal::TargetTooLong,
                    ) => "to",
                    _ => "source",
                };
                return Err(diagnostic(
                    refusal,
                    invocation
                        .and_then(|(call, plot)| located(syntax.round_trip(), call, plot, field))
                        .unwrap_or(gear.source_span),
                ));
            }
        }
    }
    Ok(())
}

fn located(source: &str, call: &Invocation, plot: &PlotSyntax, name: &str) -> Option<Span> {
    let position = if name == "source" { 0 } else { 1 };
    let mut expression = call
        .arguments
        .iter()
        .enumerate()
        .find_map(|(index, argument)| match argument {
            Argument::Named {
                name: key, value, ..
            } if key.text == name => Some(&value.syntax),
            Argument::Positional(value) if index == position => Some(&value.syntax),
            _ => None,
        })?;
    for _ in 0..=plot.back.len() {
        let ExpressionSyntax::Atomic(token) = expression else {
            return Some(expression.span());
        };
        if let Some(local) = plot.back.iter().find_map(|statement| match statement {
            BackStatement::LocalValue(local) if local.name.text == token.text => Some(local),
            _ => None,
        }) {
            expression = &local.value.syntax;
            continue;
        }
        return Some(
            QuotedTextSourceMap::new(source, token, conduit_core::EXACT_DECIMAL_MAX_LITERAL_BYTES)
                .and_then(|map| map.source_span(0..map.decoded().len()))
                .unwrap_or(token.span),
        );
    }
    None
}

fn collect<'a>(plot: &'a PlotSyntax, calls: &mut Vec<(&'a Invocation, &'a PlotSyntax)>) {
    for child in &plot.local_plots {
        collect(child, calls);
    }
    for statement in &plot.back {
        match statement {
            BackStatement::NamedGear(gear) => calls.push((&gear.invocation, plot)),
            BackStatement::Cord(cord) => stages(&cord.stages, plot, calls),
            BackStatement::MatchedRoute(route) => {
                for arm in &route.arms {
                    stages(&arm.stages, plot, calls);
                }
            }
            _ => {}
        }
    }
}
fn stages<'a>(
    stages: &'a [CordStage],
    plot: &'a PlotSyntax,
    calls: &mut Vec<(&'a Invocation, &'a PlotSyntax)>,
) {
    for stage in stages {
        if let CordStage::InlineGear(call)
        | CordStage::RelationalGear {
            invocation: call, ..
        } = stage
        {
            calls.push((call, plot));
        }
    }
}
