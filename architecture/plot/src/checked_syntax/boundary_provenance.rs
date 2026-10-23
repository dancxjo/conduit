//! Source origin of checked root Fore bindings; not inferred from flattened paths.
use crate::prelude::*;
use crate::{AuthoringFrontBinding, ExpandedConnectionOrigin, Span};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoringCordOrigin {
    pub source_span: Span,
    pub origin: ExpandedConnectionOrigin,
}

pub(crate) fn boundary_provenance_digest(
    document: &conduit_core::SourceDocumentId,
    plot: &conduit_core::CheckedPlotId,
    inputs: &[AuthoringFrontBinding],
    outputs: &[AuthoringFrontBinding],
) -> String {
    let mut canonical = String::from("canonical-boundary-provenance-v1");
    push(&mut canonical, document.as_str());
    push(&mut canonical, plot.as_str());
    let mut rows = Vec::new();
    for (direction, bindings) in [("input", inputs), ("output", outputs)] {
        for binding in bindings {
            let mut row = String::new();
            for value in [
                direction,
                binding.front_port_id.as_str(),
                binding.gear_id.as_str(),
                binding.gear_port_id.as_str(),
                binding.track.as_str(),
            ] {
                push(&mut row, value);
            }
            if let Some(origin) = &binding.source_origin {
                push(&mut row, origin.origin.as_str());
                let span = origin.source_span;
                for value in [
                    span.start,
                    span.end,
                    span.line,
                    span.column,
                    span.end_line,
                    span.end_column,
                ] {
                    push(&mut row, &value.to_string());
                }
            } else {
                push(&mut row, "unproven");
            }
            rows.push(row);
        }
    }
    rows.sort();
    for row in rows {
        push(&mut canonical, &row);
    }
    crate::hash_string(&canonical)
}

impl crate::ExpandedAuthoringPlot {
    pub fn validate_boundary_provenance(&self) -> Result<(), crate::CanonicalExpansionDiagnostic> {
        if self.boundary_provenance_digest
            != boundary_provenance_digest(
                &self.expanded.source_document_id,
                &self.expanded.checked_plot_id,
                &self.input_bindings,
                &self.output_bindings,
            )
        {
            return Err(crate::CanonicalExpansionDiagnostic::new(
                "CND-FRM-049",
                "authoring boundary provenance differs from its checked Source mapping".into(),
            ));
        }
        Ok(())
    }
}

fn push(target: &mut String, value: &str) {
    target.push_str(&value.len().to_string());
    target.push(':');
    target.push_str(value);
}
