//! Generic exact committed-edge correlation. This is preparation, not a live
//! parser authority: consumers still require the owning installed revision seam.
use crate::{parser_commitment::*, *};
use conduit_plot::{
    rust_binding::{NativeBindingRefusal, NativeRustBinding},
    PortableExpressionProgram,
};
/// One transport profile shared by Source packaging, Native preparation and tests.
pub const DEPENDENCY_QUERY_MAXIMUM_BYTES: u32 = 65536;
pub const DEPENDENCY_CONTEXT_MAXIMUM_BYTES: u32 = 5120;
pub const DEPENDENCY_RESULT_MAXIMUM_BYTES: u32 = 30720;
pub const DEPENDENCY_PAIR_MAXIMUM_BYTES: u32 = 65536;
#[derive(Debug)]
pub enum CommittedDependencyRefusal {
    Commitment(ParserCommitmentRefusal),
    Native(NativeBindingRefusal),
    UnsupportedSubtype,
    LexicalBasis,
    Program,
    Bound,
    ObservationMismatch,
}
pub struct PreparedCommittedDependency {
    commitment: PreparedParserCommitment,
    admission: LanguageCommittedDependencyAdmission,
    arc: LanguageDependencyArc,
    lexical: LanguageLexicalTape,
}
impl PreparedCommittedDependency {
    pub fn prepare(
        lexical: &lexical::PreparedLexicalTape,
        arc: &LanguageDependencyArc,
        request: &LanguageParserBeamCommitRequest,
        expected: &LanguageParserBeamCommitResult,
    ) -> Result<Self, CommittedDependencyRefusal> {
        use CommittedDependencyRefusal::*;
        let commitment =
            PreparedParserCommitment::prepare(request, expected).map_err(Commitment)?;
        if arc.relation().subtype().is_some() {
            return Err(UnsupportedSubtype);
        }
        let token = lexical
            .tape()
            .tokens()
            .iter()
            .find(|t| t.identity() == arc.dependent().token())
            .ok_or(LexicalBasis)?;
        let head = match arc.governor() {
            LanguageDependencyHead::Token(head) => {
                if !lexical
                    .tape()
                    .tokens()
                    .iter()
                    .any(|t| t.identity() == head.token())
                {
                    return Err(LexicalBasis);
                }
                *head.token().ordinal()
            }
            LanguageDependencyHead::Root => 4,
        };
        if [
            request.beam().candidate0(),
            request.beam().candidate1(),
            request.beam().candidate2(),
            request.beam().candidate3(),
        ]
        .iter()
        .any(|candidate| {
            *candidate.active()
                && *candidate.state().token_count() != lexical.tape().tokens().len() as u64
        }) {
            return Err(LexicalBasis);
        }
        let query = LanguageCommittedDependencyQuery::new(
            arc.clone(),
            commitment.request().clone(),
            head,
            lexical.tape().source().clone(),
            token.clone(),
        )
        .map_err(Native)?;
        if query.clone().encode().map_err(Native)?.len() > DEPENDENCY_QUERY_MAXIMUM_BYTES as usize
            || commitment.result().clone().encode().map_err(Native)?.len()
                > DEPENDENCY_RESULT_MAXIMUM_BYTES as usize
        {
            return Err(Bound);
        }
        let admission =
            LanguageCommittedDependencyAdmission::new(query, commitment.result().clone())
                .map_err(Native)?;
        let program = PortableExpressionProgram::from_canonical_hex(include_str!(concat!(
            env!("OUT_DIR"),
            "/committed_dependency_program.hex"
        )))
        .map_err(|_| Program)?;
        let bytes = program
            .evaluate(&admission.clone().encode().map_err(Native)?)
            .map_err(|_| Program)?;
        let arc = LanguageDependencyArc::decode(&bytes).map_err(Native)?;
        Ok(Self {
            commitment,
            admission,
            arc,
            lexical: lexical.tape().clone(),
        })
    }
    pub fn commitment(&self) -> &PreparedParserCommitment {
        &self.commitment
    }
    pub fn admission(&self) -> &LanguageCommittedDependencyAdmission {
        &self.admission
    }
    pub fn lexical(&self) -> &LanguageLexicalTape {
        &self.lexical
    }
    pub fn execution_context(
        &self,
    ) -> Result<LanguageDependencyExecutionContext, CommittedDependencyRefusal> {
        use CommittedDependencyRefusal::*;
        let program = PortableExpressionProgram::from_canonical_hex(include_str!(concat!(
            env!("OUT_DIR"),
            "/dependency_execution_context.hex"
        )))
        .map_err(|_| Program)?;
        let encoded = program
            .evaluate(&self.admission.query().clone().encode().map_err(Native)?)
            .map_err(|_| Program)?;
        if encoded.len() > DEPENDENCY_CONTEXT_MAXIMUM_BYTES as usize
            || self
                .commitment
                .result()
                .clone()
                .encode()
                .map_err(Native)?
                .len()
                > DEPENDENCY_RESULT_MAXIMUM_BYTES as usize
        {
            return Err(Bound);
        }
        LanguageDependencyExecutionContext::decode(&encoded).map_err(Native)
    }
    /// Admit Source-owned cross-field laws, then bind the raw observation to
    /// this exact replay. Serialized data alone proves neither execution nor
    /// installed currentness; the public Plan/Play path supplies execution.
    pub fn admit_execution_observation(
        &self,
        observation: &LanguageDependencyExecutionObservation,
    ) -> Result<LanguageDependencyArc, CommittedDependencyRefusal> {
        use CommittedDependencyRefusal::*;
        let admitted = LanguageDependencyExecutionResult::new(
            observation.actual().clone(),
            observation.context().clone(),
        )
        .map_err(Native)?;
        if admitted.context() != &self.execution_context()?
            || admitted.actual() != self.commitment.result()
        {
            return Err(ObservationMismatch);
        }
        let program = PortableExpressionProgram::from_canonical_hex(include_str!(concat!(
            env!("OUT_DIR"),
            "/dependency_execution_evidence.hex"
        )))
        .map_err(|_| Program)?;
        let encoded = program
            .evaluate(&admitted.encode().map_err(Native)?)
            .map_err(|_| Program)?;
        LanguageDependencyArc::decode(&encoded).map_err(Native)
    }
    pub fn arc(&self) -> &LanguageDependencyArc {
        &self.arc
    }
}

/// Exact public transport envelope, checked before optional package preparation.
pub fn committed_dependency_execution_pair() -> conduit_core::PreparedTypedTuplePairEncoder {
    let pair = conduit_core::PreparedTypedTuplePairEncoder::new(
        LanguageDependencyExecutionContext::semantic_type().expect("execution context"),
        DEPENDENCY_CONTEXT_MAXIMUM_BYTES,
        LanguageParserBeamCommitResult::semantic_type().expect("commit result"),
        DEPENDENCY_RESULT_MAXIMUM_BYTES,
    )
    .expect("bounded dependency execution pair");
    assert!(
        pair.maximum_bytes() <= DEPENDENCY_PAIR_MAXIMUM_BYTES,
        "existing finite Zip pair profile"
    );
    pair
}
/// Public Source projects a small context and executes the existing commit chain.
pub fn committed_dependency_execution_source() -> alloc::string::String {
    let pair = committed_dependency_execution_pair();
    include_str!("../dependency_commitment_execution.conduit")
        .replace(
            "DEPENDENCY_PAIR_BYTES",
            &alloc::format!("{}", pair.maximum_bytes()),
        )
        .replace(
            "DEPENDENCY_QUERY_BYTES",
            &alloc::format!("{}", DEPENDENCY_QUERY_MAXIMUM_BYTES),
        )
        .replace(
            "DEPENDENCY_CONTEXT_BYTES",
            &alloc::format!("{}", DEPENDENCY_CONTEXT_MAXIMUM_BYTES),
        )
        .replace(
            "DEPENDENCY_RESULT_BYTES",
            &alloc::format!("{}", DEPENDENCY_RESULT_MAXIMUM_BYTES),
        )
}
