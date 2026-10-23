//! Cord type/stage checking using lexical resolution and leaf invocation checks.
use super::{check_invocation, structured_selector, use_diagnostic, Resolver};
use crate::prelude::*;
use crate::{
    CanonicalStartupValue, CheckedCanonicalGear, CheckedCordStage, CordStage, KindSignature,
    PlotSyntax, StartupCatalog, SyntaxCheckDiagnostic, SyntaxCheckError,
};
use alloc::collections::BTreeMap;

pub(super) fn check_direct_cord_type(
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

pub(super) fn check_cord_stages(
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
