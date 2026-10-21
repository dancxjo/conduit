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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedCanonicalCord {
    pub stages: Vec<CheckedCordStage>,
}
