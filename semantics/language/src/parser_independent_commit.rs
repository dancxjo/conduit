//! Unwired independent dependency commit preparation; Source wiring and typed
//! transaction custody gates are required before production publication.
//! Protection and complete speech coverage cannot construct this private receipt.
use crate::{
    LanguageParserIndependentCommitRequest, LanguageParserIndependentCommitSet,
    LanguageParserIndependentCommitSetProposal,
};
use alloc::vec::Vec;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};
use conduit_plot::PortableExpressionProgram;

#[derive(Debug)]
pub enum IndependentCommitRefusal {
    Native(NativeBindingRefusal),
    Program,
}
/// Complete Source commit request and exact execution, immutable after admission.
/// No parser contiguous-frontier or playback acknowledgement is implied.
pub struct PreparedIndependentCommittedDependency {
    request: LanguageParserIndependentCommitRequest,
    committed: LanguageParserIndependentCommitSet,
    program: PortableExpressionProgram,
    input: Vec<u8>,
    output: Vec<u8>,
}
impl PreparedIndependentCommittedDependency {
    /// The aggregate owner supplies its exact privately retained previous set,
    /// pre-reserves custody, retains all original facts/rebases, then publishes.
    pub(crate) fn prepare(
        request: LanguageParserIndependentCommitRequest,
    ) -> Result<Self, IndependentCommitRefusal> {
        let program = PortableExpressionProgram::from_canonical_hex(include_str!(concat!(
            env!("OUT_DIR"),
            "/parser_independent_commit.hex"
        )))
        .map_err(|_| IndependentCommitRefusal::Program)?;
        let input = request
            .clone()
            .encode()
            .map_err(IndependentCommitRefusal::Native)?;
        let output = program
            .evaluate(&input)
            .map_err(|_| IndependentCommitRefusal::Program)?;
        let proposal = LanguageParserIndependentCommitSetProposal::decode(&output)
            .map_err(IndependentCommitRefusal::Native)?;
        let committed = LanguageParserIndependentCommitSet::new(
            *proposal.active(),
            proposal.basis().clone(),
            proposal.edge0().clone(),
            proposal.edge1().clone(),
            proposal.edge2().clone(),
            proposal.edge3().clone(),
        )
        .map_err(IndependentCommitRefusal::Native)?;
        Ok(Self {
            request,
            committed,
            program,
            input,
            output,
        })
    }
    pub fn request(&self) -> &LanguageParserIndependentCommitRequest {
        &self.request
    }
    pub fn committed(&self) -> &LanguageParserIndependentCommitSet {
        &self.committed
    }
    pub fn program(&self) -> &PortableExpressionProgram {
        &self.program
    }
    pub fn source_input(&self) -> &[u8] {
        &self.input
    }
    pub fn source_output(&self) -> &[u8] {
        &self.output
    }
}
