//! Source origin of one exact ordered expanded connection.
use crate::prelude::*;
use crate::{CheckedConnection, Span};
use conduit_core::{CheckedPlotId, SourceDocumentId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpandedConnectionOrigin {
    /// Wiring between checked Cord stages; normalization may have produced stages.
    CordStages,
    /// Extra input wiring introduced by a relational Gear expression.
    RelationalOperand,
    /// Internal wiring introduced while lowering a semantic expression.
    SemanticExpression,
}

impl ExpandedConnectionOrigin {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CordStages => "cord-stages",
            Self::RelationalOperand => "relational-operand",
            Self::SemanticExpression => "semantic-expression",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpandedConnectionProvenance {
    /// Exact endpoints, track, Kind and temporal facts, never an inferred path prefix.
    pub connection: CheckedConnection,
    pub source_document_id: SourceDocumentId,
    pub checked_plot_id: CheckedPlotId,
    pub source_plot: String,
    pub plot_path: Vec<String>,
    /// Originating checked Cord or route arm, including for generated wiring.
    pub source_span: Span,
    pub origin: ExpandedConnectionOrigin,
}

pub(crate) fn connection_key(connection: &CheckedConnection) -> String {
    let mut key = String::new();
    for value in [
        connection.source_gear_id.as_str(),
        connection.source_port_id.as_str(),
        connection.sink_gear_id.as_str(),
        connection.sink_port_id.as_str(),
        connection.value_kind.as_str(),
        connection.track.as_str(),
        connection.temporal.as_str(),
    ] {
        push(&mut key, value);
    }
    key
}

pub(crate) fn provenance_key(row: &ExpandedConnectionProvenance) -> String {
    let mut key = connection_key(&row.connection);
    for value in [
        row.source_document_id.as_str(),
        row.checked_plot_id.as_str(),
        row.source_plot.as_str(),
        row.origin.as_str(),
    ] {
        push(&mut key, value);
    }
    push(&mut key, &row.plot_path.len().to_string());
    for segment in &row.plot_path {
        push(&mut key, segment);
    }
    for value in [
        row.source_span.start,
        row.source_span.end,
        row.source_span.line,
        row.source_span.column,
        row.source_span.end_line,
        row.source_span.end_column,
    ] {
        push(&mut key, &value.to_string());
    }
    key
}

pub(crate) fn validate_connection_provenance(
    connections: &[CheckedConnection],
    provenance: &[ExpandedConnectionProvenance],
) -> Result<(), crate::CanonicalExpansionDiagnostic> {
    let mut actual = connections.iter().map(connection_key).collect::<Vec<_>>();
    let mut mapped = provenance
        .iter()
        .map(|row| connection_key(&row.connection))
        .collect::<Vec<_>>();
    actual.sort();
    mapped.sort();
    if actual != mapped
        || provenance
            .iter()
            .any(|row| row.source_span.start > row.source_span.end)
    {
        return Err(crate::CanonicalExpansionDiagnostic::new(
            "CND-FRM-049",
            "connection provenance does not cover the exact expanded graph".into(),
        ));
    }
    Ok(())
}

fn push(target: &mut String, value: &str) {
    target.push_str(&value.len().to_string());
    target.push(':');
    target.push_str(value);
}
