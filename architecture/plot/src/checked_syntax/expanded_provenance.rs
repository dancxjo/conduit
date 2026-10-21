//! Expanded Source provenance and its canonical identity encoding.
//! Depends only on provenance data, lexical spans and the shared hash primitive.
use crate::prelude::*;
use crate::{hash_string, Span};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpandedGearProvenance {
    /// Compiler-owned validation scope; authored gear paths cannot grant it.
    pub(crate) construction_owner: Option<conduit_core::KindId>,
    pub gear_id: String,
    pub plot_path: Vec<String>,
    pub source_plot: String,
    pub source_gear: String,
    pub source_span: Span,
}

pub(crate) fn provenance_digest(
    source_document_id: &conduit_core::SourceDocumentId,
    provenance: &[ExpandedGearProvenance],
) -> String {
    let mut canonical = String::from("canonical-expansion-provenance");
    push(&mut canonical, source_document_id.as_str());
    for row in provenance {
        push(&mut canonical, &row.gear_id);
        push(&mut canonical, &row.plot_path.join("/"));
        push(&mut canonical, &row.source_plot);
        push(&mut canonical, &row.source_gear);
        push(&mut canonical, &row.source_span.start.to_string());
        push(&mut canonical, &row.source_span.end.to_string());
        push(&mut canonical, &row.source_span.line.to_string());
        push(&mut canonical, &row.source_span.column.to_string());
    }
    hash_string(&canonical)
}

fn push(target: &mut String, value: &str) {
    target.push_str(&value.len().to_string());
    target.push(':');
    target.push_str(value);
}
