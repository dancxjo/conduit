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
