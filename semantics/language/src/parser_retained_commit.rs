//! Unwired typed retained-commit preparation. Generated Source prerequisites
//! and complete Session transaction tests are required before integration.
//! A public compact anchor is not sufficient authority: retain the original
//! Native commit query and its exact fixed Source projection execution.
use crate::{
    LanguageParserJointBeam, LanguageParserJointCommitQuery, LanguageParserRetainedCommitAnchor,
    LanguageParserRetainedCommitAnchorProposal, LanguageParserRetainedCommitReceipt,
};
use alloc::vec::Vec;
use conduit_core::StructuredInfoValue;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};
use conduit_plot::PortableExpressionProgram;

pub(crate) enum RetainedCommitRefusal {
    Native(NativeBindingRefusal),
    Source,
}
impl core::fmt::Debug for RetainedCommitRefusal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Native(v) => f.debug_tuple("Native").field(v).finish(),
            Self::Source => f.write_str("Source"),
        }
    }
}
impl From<NativeBindingRefusal> for RetainedCommitRefusal {
    fn from(v: NativeBindingRefusal) -> Self {
        Self::Native(v)
    }
}
/// Pure preparation; this neither executes a commitment Flow nor publishes a
/// parser frontier. The Session must reserve finite resources before Flow.
pub(crate) struct PreparedRetainedCommit {
    query: LanguageParserJointCommitQuery,
    anchor: LanguageParserRetainedCommitAnchor,
    program: PortableExpressionProgram,
    input: Vec<u8>,
    output: Vec<u8>,
}
impl PreparedRetainedCommit {
    pub(crate) fn prepare(
        query: LanguageParserJointCommitQuery,
    ) -> Result<Self, RetainedCommitRefusal> {
        let program = PortableExpressionProgram::from_canonical_hex(include_str!(concat!(
            env!("OUT_DIR"),
            "/parser_retained_commit_anchor.hex"
        )))
        .map_err(|_| RetainedCommitRefusal::Source)?;
        let input = query
            .clone()
            .into_structured()?
            .canonical_bytes()
            .map_err(|_| RetainedCommitRefusal::Source)?;
        let output = program
            .evaluate(&input)
            .map_err(|_| RetainedCommitRefusal::Source)?;
        let raw = LanguageParserRetainedCommitAnchorProposal::from_structured(
            StructuredInfoValue::from_canonical_bytes(&output)
                .map_err(|_| RetainedCommitRefusal::Source)?,
        )?;
        // Structural extraction only. Source NativeAnchor owns bounded ordinal
        // admission; no nominal ID recast and no host commitment predicate.
        let anchor =
            LanguageParserRetainedCommitAnchor::new(raw.basis().clone(), *raw.dependent())?;
        Ok(Self {
            query,
            anchor,
            program,
            input,
            output,
        })
    }
    pub(crate) fn query(&self) -> &LanguageParserJointCommitQuery {
        &self.query
    }
    pub(crate) fn anchor(&self) -> &LanguageParserRetainedCommitAnchor {
        &self.anchor
    }
    pub(crate) fn program(&self) -> &PortableExpressionProgram {
        &self.program
    }
    pub(crate) fn source_input(&self) -> &[u8] {
        &self.input
    }
    pub(crate) fn source_output(&self) -> &[u8] {
        &self.output
    }
    pub(crate) fn admit_output(
        &self,
        next: LanguageParserJointBeam,
    ) -> Result<LanguageParserRetainedCommitReceipt, RetainedCommitRefusal> {
        Ok(LanguageParserRetainedCommitReceipt::new(
            self.anchor.clone(),
            next,
            self.query.fact().query().beam().clone(),
        )?)
    }
}
