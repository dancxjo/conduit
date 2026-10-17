//! Correlate domain preparation refusals with the exact original quoted Source.
use super::{
    contract, prepare_configuration, IpaConstructor, IpaConstructorDiagnostic,
    IpaConstructorLocation, IpaConstructorRefusal,
};
use alloc::{boxed::Box, vec::Vec};

mod location;
use conduit_core::{ConfigurationEntry, SourceDocumentId};
use conduit_plot::{
    BackStatement, CanonicalStartupValue, CheckedSyntaxDocument, CordStage, Invocation, PlotSyntax,
    Span, SyntaxDocument,
};
use location::located;

#[derive(Debug)]
pub struct IpaSourceDiagnostic {
    pub source_document_id: SourceDocumentId,
    pub span: Span,
    pub cause: Box<IpaConstructorDiagnostic>,
}
impl core::fmt::Display for IpaSourceDiagnostic {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            formatter,
            "CND-SPC-IPA at {}:{}..{}:{}: {:?}",
            self.span.line,
            self.span.column,
            self.span.end_line,
            self.span.end_column,
            self.cause.refusal
        )
    }
}
/// Concrete authored requests execute the same preparation used by installed
/// Backs. Parameterized candidates remain unresolved until concrete preparation;
/// this function never presents them as membership receipts.
pub fn validate_source(
    syntax: &SyntaxDocument,
    checked: &CheckedSyntaxDocument,
) -> Result<(), IpaSourceDiagnostic> {
    let same_source = syntax.source_document_id() == checked.source_document_id;
    let mut invocations = Vec::new();
    for plot in &syntax.plots {
        collect(plot, &mut invocations);
    }
    for plot in &checked.plots {
        for gear in &plot.gears {
            let Some(constructor) = IpaConstructor::from_kind(&gear.kind) else {
                continue;
            };
            let diagnostic = |cause, span| IpaSourceDiagnostic {
                source_document_id: checked.source_document_id.clone(),
                span,
                cause: Box::new(cause),
            };
            if !same_source {
                return Err(diagnostic(
                    IpaConstructorDiagnostic {
                        refusal: IpaConstructorRefusal::SourceCorrelation,
                        location: Box::new(IpaConstructorLocation::Request),
                    },
                    gear.source_span,
                ));
            }
            let invocation = invocations
                .iter()
                .copied()
                .find(|(invocation, _)| invocation.span == gear.source_span);
            let mut configuration = Vec::new();
            let fields = contract(constructor).configuration;
            let mut unresolved = false;
            for binding in &gear.startup_bindings {
                if !matches!(&binding.value, CanonicalStartupValue::Structured(value) if value.try_concrete().is_some())
                {
                    unresolved = true;
                    break;
                }
                let Some(field) = fields.iter().find(|field| field.key == binding.name) else {
                    continue;
                };
                let value =
                    conduit_plot::validate_startup_configuration(field, binding.value.clone())
                        .map_err(|error| {
                            let path = constructor
                                .parameters()
                                .into_iter()
                                .find(|(name, _, _)| *name == binding.name)
                                .expect("reviewed constructor parameter")
                                .0;
                            let location = IpaConstructorLocation::Field(path);
                            diagnostic(
                                IpaConstructorDiagnostic {
                                    refusal: IpaConstructorRefusal::Startup(error),
                                    location: Box::new(location.clone()),
                                },
                                invocation
                                    .and_then(|(invocation, plot)| {
                                        located(syntax.round_trip(), invocation, plot, &location)
                                    })
                                    .unwrap_or(gear.source_span),
                            )
                        })?;
                configuration.push(ConfigurationEntry {
                    key: binding.name.clone(),
                    value,
                });
            }
            if unresolved {
                continue;
            }
            if let Err(cause) = prepare_configuration(constructor, &configuration) {
                let span = invocation
                    .and_then(|(invocation, plot)| {
                        located(syntax.round_trip(), invocation, plot, &cause.location)
                    })
                    .unwrap_or(gear.source_span);
                return Err(diagnostic(cause, span));
            }
        }
    }
    Ok(())
}
fn collect<'a>(plot: &'a PlotSyntax, invocations: &mut Vec<(&'a Invocation, &'a PlotSyntax)>) {
    for child in &plot.local_plots {
        collect(child, invocations);
    }
    for statement in &plot.back {
        match statement {
            BackStatement::NamedGear(gear) => invocations.push((&gear.invocation, plot)),
            BackStatement::Cord(cord) => stages(&cord.stages, plot, invocations),
            BackStatement::MatchedRoute(route) => {
                for arm in &route.arms {
                    stages(&arm.stages, plot, invocations);
                }
            }
            _ => {}
        }
    }
}
fn stages<'a>(
    stages: &'a [CordStage],
    plot: &'a PlotSyntax,
    invocations: &mut Vec<(&'a Invocation, &'a PlotSyntax)>,
) {
    for stage in stages {
        match stage {
            CordStage::InlineGear(invocation) | CordStage::RelationalGear { invocation, .. } => {
                invocations.push((invocation, plot))
            }
            _ => {}
        }
    }
}
