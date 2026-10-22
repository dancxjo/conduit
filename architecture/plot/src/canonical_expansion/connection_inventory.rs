//! Keep origin attached while recursive expansion merges connection inventories.
use super::*;
use crate::{ExpandedConnectionOrigin, ExpandedConnectionProvenance};

#[derive(Debug)]
struct Origin {
    document: Option<conduit_core::SourceDocumentId>,
    plot: conduit_core::CheckedPlotId,
    name: String,
    path: Vec<String>,
    span: crate::Span,
}

#[derive(Debug, Default)]
pub(super) struct ConnectionInventory {
    connections: Vec<CheckedConnection>,
    origins: Vec<(ExpandedConnectionOrigin, Option<Origin>)>,
}

impl core::ops::Deref for ConnectionInventory {
    type Target = [CheckedConnection];
    fn deref(&self) -> &Self::Target {
        &self.connections
    }
}

impl ConnectionInventory {
    pub(super) fn push_generated(&mut self, connection: CheckedConnection) {
        self.push_with_origin(connection, ExpandedConnectionOrigin::SemanticExpression);
    }

    pub(super) fn push_with_origin(
        &mut self,
        connection: CheckedConnection,
        kind: ExpandedConnectionOrigin,
    ) {
        self.connections.push(connection);
        self.origins.push((kind, None));
    }

    pub(super) fn extend(&mut self, other: Self) {
        self.connections.extend(other.connections);
        self.origins.extend(other.origins);
    }

    /// Child rows already have their lexical owner; only this Cord's new rows
    /// receive its location. Endpoints do not participate in owner discovery.
    pub(super) fn attribute_since(
        &mut self,
        start: usize,
        plot: &CheckedCanonicalPlot,
        path: &[String],
        span: crate::Span,
    ) {
        for (_, origin) in &mut self.origins[start..] {
            if origin.is_none() {
                *origin = Some(Origin {
                    document: None,
                    plot: plot.checked_plot_id.clone(),
                    name: plot.name.clone(),
                    path: path.to_vec(),
                    span,
                });
            }
        }
    }

    /// A selected Back closes its own lexical document before merging. The
    /// root closes remaining local Plot origins; nested Back documents survive.
    pub(super) fn bind_document(&mut self, document: &conduit_core::SourceDocumentId) {
        for (_, origin) in &mut self.origins {
            if let Some(origin) = origin {
                origin.document.get_or_insert_with(|| document.clone());
            }
        }
    }

    pub(super) fn finish(
        mut self,
        document: &conduit_core::SourceDocumentId,
    ) -> Result<
        (Vec<CheckedConnection>, Vec<ExpandedConnectionProvenance>),
        CanonicalExpansionDiagnostic,
    > {
        self.bind_document(document);
        let mut provenance = Vec::with_capacity(self.connections.len());
        for (connection, (kind, origin)) in self.connections.iter().zip(self.origins) {
            let origin = origin.ok_or_else(|| {
                CanonicalExpansionDiagnostic::new(
                    "CND-FRM-049",
                    "expanded connection lost its checked Source origin".into(),
                )
            })?;
            provenance.push(ExpandedConnectionProvenance {
                connection: connection.clone(),
                source_document_id: origin.document.expect("document bound above"),
                checked_plot_id: origin.plot,
                source_plot: origin.name,
                plot_path: origin.path,
                source_span: origin.span,
                origin: kind,
            });
        }
        Ok((self.connections, provenance))
    }
}
