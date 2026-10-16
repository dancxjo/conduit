//! Declared per-frame profiles of existing pure DSP Source, never a DSP engine.
use alloc::{format, string::String};
use conduit_plot::{
    parse_syntax_document, PlotCompletionPolicy, RuntimePortDirection, RuntimePortTemporal,
};

pub(super) fn value_profile(source: &str, owner: &str, name: &str) -> Result<String, String> {
    let document = parse_syntax_document(source);
    if !document.diagnostics.is_empty() {
        return Err(format!("DSP profile owner does not parse: {owner}"));
    }
    let plot = document
        .plots
        .iter()
        .find(|p| p.name.text == owner)
        .ok_or_else(|| format!("DSP profile owner is missing: {owner}"))?;
    let body = plot
        .expression_body
        .as_ref()
        .ok_or_else(|| format!("DSP Value profile requires one pure expression: {owner}"))?;
    if plot.completion != PlotCompletionPolicy::Live
        || plot.front.shorthand.is_some()
        || !plot.front.type_parameters.is_empty()
        || !plot.front.kind_parameters.is_empty()
        || !plot.front.startup_parameters.is_empty()
        || !plot.local_plots.is_empty()
        || plot.front.runtime_ports.len() != 2
    {
        return Err(format!("unsupported DSP Value profile Fore: {owner}"));
    }
    let mut input = None;
    let mut output = None;
    for port in &plot.front.runtime_ports {
        if port.temporal != (RuntimePortTemporal::Flow { closes: true })
            || port.maximum_bytes.is_some()
            || !port.refinements.is_empty()
        {
            return Err(format!(
                "DSP Value profile cannot erase additional port contracts: {owner}"
            ));
        }
        let slot = match port.direction {
            RuntimePortDirection::Input => &mut input,
            RuntimePortDirection::Output => &mut output,
        };
        if slot.replace(port).is_some() {
            return Err(format!(
                "DSP Value profile requires one input and output: {owner}"
            ));
        }
    }
    let input = input.ok_or_else(|| format!("DSP profile input missing: {owner}"))?;
    let output = output.ok_or_else(|| format!("DSP profile output missing: {owner}"))?;
    let span = body.expression.span;
    let expression = source
        .get(span.start..span.end)
        .ok_or_else(|| format!("DSP profile expression span changed: {owner}"))?;
    Ok(format!(
        "# Generated Value profile of {owner}; owner-source {}\nplot {name} (\n >> {}: {}\n {}: {} >>\n) = ({expression})\n",
        document.source_document_id().as_str(), input.name.text, input.value_type.text, output.name.text, output.value_type.text,
    ))
}

/// Carry context alongside the unchanged expression, rebinding only input paths.
pub(super) fn carried_profile(source: &str, owner: &str, name: &str) -> Result<String, String> {
    carried(source, owner, name, false)
}
/// Explicitly name the owned mixer's two tuple inputs left/right in its carrier.
pub(super) fn carried_pair_profile(
    source: &str,
    owner: &str,
    name: &str,
) -> Result<String, String> {
    carried(source, owner, name, true)
}
fn carried(source: &str, owner: &str, name: &str, named_pair: bool) -> Result<String, String> {
    let value = value_profile(source, owner, "dsp/carried-profile-template")?;
    let document = parse_syntax_document(&value);
    let expression = &document.plots[0]
        .expression_body
        .as_ref()
        .unwrap()
        .expression;
    let mut edits = alloc::vec::Vec::new();
    projected_inputs(&expression.syntax, named_pair, &mut edits)?;
    edits.sort_unstable_by_key(|edit| core::cmp::Reverse(edit.0.start));
    edits.dedup();
    let mut body = String::from(&value[expression.span.start..expression.span.end]);
    for (span, expected, replacement) in edits {
        if value.get(span.start..span.end) != Some(expected) {
            return Err(format!("DSP carried input span changed: {owner}"));
        }
        body.replace_range(
            span.start - expression.span.start..span.end - expression.span.start,
            replacement,
        );
    }
    let mapping = if named_pair {
        "; tuple 0/1 map to control.left/right"
    } else {
        ""
    };
    Ok(format!(
        "# Context-carrying Value profile of {owner}{mapping}; numeric Source unchanged\nplot {name} (\n item: type\n output: type\n >> value: item\n result: output >>\n) = ({{context: .context, outcome: ({body})}})\n"
    ))
}
type InputEdit = (conduit_plot::Span, &'static str, &'static str);
fn projected_inputs(
    node: &conduit_plot::ExpressionSyntax,
    named_pair: bool,
    edits: &mut alloc::vec::Vec<InputEdit>,
) -> Result<(), String> {
    use conduit_plot::ExpressionSyntax as E;
    match node {
        E::Projection { value, member, .. } => {
            if let E::Input(span) = &**value {
                edits.push((*span, ".", ".control."));
                if named_pair {
                    let conduit_plot::ExpressionProjection::TupleIndex(index) = member else {
                        return Err(String::from("DSP pair profile requires tuple inputs"));
                    };
                    edits.push(match index.text.as_str() {
                        "0" => (index.span, "0", "left"),
                        "1" => (index.span, "1", "right"),
                        _ => {
                            return Err(String::from("DSP pair profile admits exactly two inputs"))
                        }
                    });
                }
            } else {
                projected_inputs(value, named_pair, edits)?;
            }
        }
        E::Unary { operand, .. } => projected_inputs(operand, named_pair, edits)?,
        E::Binary { left, right, .. } => {
            projected_inputs(left, named_pair, edits)?;
            projected_inputs(right, named_pair, edits)?;
        }
        E::Conditional {
            condition,
            when_true,
            when_false,
            ..
        } => {
            projected_inputs(condition, named_pair, edits)?;
            projected_inputs(when_true, named_pair, edits)?;
            projected_inputs(when_false, named_pair, edits)?;
        }
        E::Tuple { values, .. }
        | E::Collection { values, .. }
        | E::SemanticCall {
            arguments: values, ..
        } => {
            for value in values {
                projected_inputs(value, named_pair, edits)?;
            }
        }
        E::Record { fields, .. } => {
            for field in fields {
                if field.punned {
                    return Err(String::from(
                        "DSP carried profile cannot rebind punned fields",
                    ));
                }
                projected_inputs(&field.value, named_pair, edits)?;
            }
        }
        E::Variant { payload, .. } => projected_inputs(payload, named_pair, edits)?,
        E::Atomic(_) => {}
        E::Input(_) | E::TypedGlyphLiteral(_) => {
            return Err(String::from(
                "DSP carried profile requires ordinary projected inputs",
            ))
        }
    }
    Ok(())
}
