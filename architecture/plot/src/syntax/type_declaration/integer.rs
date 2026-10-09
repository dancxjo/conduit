//! Authored finite integer expressions; spans retain the original spelling.
use super::{Span, SpannedText};
use crate::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeIntegerOperator {
    Add,
    Multiply,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeIntegerExpressionSyntax {
    Literal {
        value: u16,
        span: Span,
    },
    Parameter(SpannedText),
    Binary {
        operator: NativeIntegerOperator,
        left: Box<Self>,
        right: Box<Self>,
        operator_span: Span,
        span: Span,
    },
    Group {
        value: Box<Self>,
        span: Span,
    },
}

impl NativeIntegerExpressionSyntax {
    pub fn span(&self) -> Span {
        match self {
            Self::Literal { span, .. } | Self::Binary { span, .. } | Self::Group { span, .. } => {
                *span
            }
            Self::Parameter(name) => name.span,
        }
    }

    pub fn literal_value(&self) -> Option<u16> {
        match self {
            Self::Literal { value, .. } => Some(*value),
            _ => None,
        }
    }
}
