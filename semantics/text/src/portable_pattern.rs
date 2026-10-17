//! Ordinary pattern specifications and their finite checked constraint consumer.
//! No automaton or matching work is admitted until the consumer supplies a bound.

use alloc::string::String;
use conduit_core::ValueConstraint;
use conduit_plot::{
    parse_text_pattern, rust_binding::NativeBindingRefusal, TextPatternDefinitionError,
    TextPatternSourceError,
};

use crate::PortablePatternSpecification;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortablePatternSpecificationRefusal {
    Native(NativeBindingRefusal),
    Source(TextPatternSourceError),
    Definition(TextPatternDefinitionError),
}

impl PortablePatternSpecification {
    /// Explicit construction with the same payload and flags as a portable
    /// slash pattern. Source is raw inner expression text, without anchors or
    /// generic string unescaping; anchor meaning is supplied explicitly.
    pub fn new(
        source: String,
        case_insensitive: bool,
        anchored_start: bool,
        anchored_end: bool,
    ) -> Result<Self, PortablePatternSpecificationRefusal> {
        parse_text_pattern(&source).map_err(PortablePatternSpecificationRefusal::Source)?;
        Self::new_native(anchored_end, anchored_start, case_insensitive, source)
            .map_err(PortablePatternSpecificationRefusal::Native)
    }

    /// Recheck received specification bytes and compile through the established
    /// portable pattern law. An admitted specification does not invent a finite
    /// input bound, and shape decoding alone does not admit a matching machine.
    pub fn checked_constraint(
        &self,
        maximum_input_bytes: u32,
        negated: bool,
    ) -> Result<ValueConstraint, PortablePatternSpecificationRefusal> {
        let mut expression = parse_text_pattern(self.source())
            .map_err(PortablePatternSpecificationRefusal::Source)?;
        if *self.case_insensitive() {
            expression = expression.ascii_case_insensitive();
        }
        let pattern = expression
            .compile_search(maximum_input_bytes)
            .map_err(PortablePatternSpecificationRefusal::Definition)?;
        Ok(ValueConstraint::TextPattern {
            pattern,
            anchored_start: *self.anchored_start(),
            anchored_end: *self.anchored_end(),
            negated,
        })
    }
}

/// Install the source-owned Type and its nested refinement contracts. This does
/// not install notation syntax, a matcher Back, or any runtime parsing offer.
pub fn install_portable_pattern_type(
    startup: &mut conduit_plot::StartupCatalog,
) -> Result<(), String> {
    let source = include_str!("../pattern-types.conduit");
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(source),
        &conduit_plot::StartupCatalog::new(),
    )
    .map_err(|error| alloc::format!("portable pattern Type: {error:?}"))?;
    let ty = &checked.native_types[0];
    startup.insert_checked_native_type("PortablePatternSpecification", ty)?;
    startup.insert_checked_native_type("pattern/portable/specification", ty)
}
