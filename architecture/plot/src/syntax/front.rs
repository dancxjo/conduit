//! Callable Plot Front parameters, ports and finite value refinements.
use super::{Expression, SpannedText};
use crate::prelude::*;
use crate::Span;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlotFront {
    pub type_parameters: Vec<TypeParameter>,
    pub kind_parameters: Vec<KindParameter>,
    pub startup_parameters: Vec<StartupParameter>,
    pub runtime_ports: Vec<RuntimePort>,
    pub shorthand: Option<ShorthandPair>,
    pub span: Option<Span>,
}

/// One exact compile-time Kind or checked source Plot parameter.
///
/// The parameter disappears during specialization. Its Fore is a semantic
/// compatibility constraint, never a runtime callable value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KindParameter {
    pub name: SpannedText,
    pub front: PlotFront,
    pub span: Span,
}

/// One compile-time checked type name. It is never a startup value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeParameter {
    pub name: SpannedText,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupParameter {
    pub name: SpannedText,
    pub value_type: SpannedText,
    pub optional: bool,
    pub maximum_bytes: Option<u64>,
    pub refinements: Vec<ValueRefinement>,
    pub default: Option<Expression>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimePortDirection {
    Input,
    Output,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimePortTemporal {
    Value,
    OptionalValue,
    Flow { closes: bool },
    Current,
    CurrentOptional,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePort {
    pub name: SpannedText,
    pub value_type: SpannedText,
    pub direction: RuntimePortDirection,
    pub temporal: RuntimePortTemporal,
    pub maximum_bytes: Option<u64>,
    pub refinements: Vec<ValueRefinement>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueRefinement {
    Finite {
        span: Span,
    },
    TextPattern {
        glyph: Option<Box<Expression>>,
        source: SpannedText,
        case_insensitive: bool,
        anchored_start: bool,
        anchored_end: bool,
        negated: bool,
        span: Span,
    },
    Range {
        minimum: Option<SpannedText>,
        maximum: Option<SpannedText>,
        minimum_endpoint: RefinementIntervalEndpoint,
        maximum_endpoint: RefinementIntervalEndpoint,
        span: Span,
    },
    Membership {
        members: Vec<SpannedText>,
        negated: bool,
        span: Span,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefinementIntervalEndpoint {
    Inclusive,
    Exclusive,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShorthandPair {
    pub input_port: SpannedText,
    pub output_port: SpannedText,
    pub span: Span,
}
