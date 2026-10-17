//! Source locations follow checked immutable locals, never ambient names.
use super::IpaConstructorLocation;
use conduit_plot::{
    Argument, BackStatement, ExpressionSyntax, Invocation, PlotSyntax, QuotedTextSourceMap, Span,
};

pub(super) fn located(
    source: &str,
    invocation: &Invocation,
    plot: &PlotSyntax,
    location: &IpaConstructorLocation,
) -> Option<Span> {
    let path = match location {
        IpaConstructorLocation::Request => "request",
        IpaConstructorLocation::Field(path) => path,
        IpaConstructorLocation::Original(_) => "request.original",
    };
    let mut parts = path.split('.');
    let name = parts.next()?;
    let mut expression = invocation
        .arguments
        .iter()
        .find_map(|argument| match argument {
            Argument::Named {
                name: argument_name,
                value,
                ..
            } if argument_name.text == name => Some(&value.syntax),
            Argument::Positional(value) if name == "request" => Some(&value.syntax),
            _ => None,
        })?;
    for part in parts {
        expression = resolve_local(expression, plot)?;
        let ExpressionSyntax::Record { fields, .. } = expression else {
            return Some(expression.span());
        };
        expression = &fields.iter().find(|field| field.name.text == part)?.value;
    }
    expression = resolve_local(expression, plot)?;
    if let (IpaConstructorLocation::Original(span), ExpressionSyntax::Atomic(token)) =
        (location, expression)
    {
        if let Some(map) = QuotedTextSourceMap::new(source, token, 4096) {
            return map.source_span(span.byte_start..span.byte_end);
        }
    }
    Some(expression.span())
}

fn resolve_local<'a>(
    mut expression: &'a ExpressionSyntax,
    plot: &'a PlotSyntax,
) -> Option<&'a ExpressionSyntax> {
    // At most one hop per declared local. The checker rejects dependency cycles;
    // this independent bound prevents diagnostics from following a cycle anyway.
    for _ in 0..=plot.back.len() {
        let ExpressionSyntax::Atomic(token) = expression else {
            return Some(expression);
        };
        let local = plot.back.iter().find_map(|statement| match statement {
            BackStatement::LocalValue(local) if local.name.text == token.text => Some(local),
            _ => None,
        });
        let Some(local) = local else {
            return Some(expression);
        };
        expression = &local.value.syntax;
    }
    None
}
