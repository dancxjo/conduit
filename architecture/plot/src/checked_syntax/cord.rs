//! Checked Cord data, distinct from flattened runtime connections.
use super::{CanonicalStartupValue, CheckedCanonicalGear};
use crate::prelude::*;
use crate::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedCordStage {
    Reference(String),
    RelationalGear {
        operands: Vec<String>,
        gear: CheckedCanonicalGear,
        input_ports: Vec<String>,
        output_port: String,
    },
    TerminalProjection {
        endpoint: String,
        terminal: crate::TerminalProjection,
        source_span: Span,
    },
    Cancellation {
        gear: String,
        source_span: Span,
    },
    When {
        expression: crate::ExpressionSyntax,
        source_span: Span,
    },
    PureExpression {
        expression: crate::ExpressionSyntax,
        source_span: Span,
    },
    InlineGear(CheckedCanonicalGear),
    Literal {
        value: CanonicalStartupValue,
        source_span: Span,
    },
    StructuredSelector {
        selector: conduit_core::StructuredSelector,
        source_span: Span,
    },
}

#[derive(Debug, Clone)]
pub struct CheckedCanonicalCord {
    /// Whole Cord or matched-route arm location in its checked Source document.
    /// Location metadata does not participate in semantic equality or identity.
    pub source_span: Span,
    pub stages: Vec<CheckedCordStage>,
}

impl PartialEq for CheckedCanonicalCord {
    fn eq(&self, other: &Self) -> bool {
        self.stages == other.stages
    }
}

impl Eq for CheckedCanonicalCord {}

#[cfg(test)]
mod tests {
    #[test]
    fn reference_cord_retains_source_extent_without_changing_semantics() {
        let source = "plot transform (\n input: Text >> output: Text\n) {\n input >> output\n}\n";
        let parsed = crate::parse_syntax_document(source);
        let checked = crate::check_syntax_document(&parsed, &crate::StartupCatalog::new())
            .expect("reference-only Cord checks");
        let cord = &checked.plots[0].cords[0];
        assert_eq!(
            source[cord.source_span.start..cord.source_span.end].trim(),
            "input >> output"
        );
        let mut relocated = cord.clone();
        relocated.source_span.start += 1;
        assert_eq!(cord, &relocated);
        assert_eq!(
            crate::syntax_identity::canonical_cord(cord),
            crate::syntax_identity::canonical_cord(&relocated)
        );
    }
}
