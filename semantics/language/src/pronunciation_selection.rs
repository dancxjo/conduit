//! Finite source-owned selection conditioned on an exact supplied canonical arc.
//! This admits profile policy; it makes no claim that a parser found the arc.
use crate::*;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};

#[derive(Debug)]
pub enum PronunciationSelectionRefusal {
    Token,
    Arc,
    Policy,
    Ambiguity,
    Program,
    Native(NativeBindingRefusal),
}
pub struct PreparedPronunciationSelection {
    request: LanguagePronunciationSelectionRequest,
    lexical_profile: LanguageLexicalProfile,
    candidate: LanguageLexicalCandidate,
    candidate_selection: LanguagePronunciationCandidateResult,
}
impl PreparedPronunciationSelection {
    pub fn request(&self) -> &LanguagePronunciationSelectionRequest {
        &self.request
    }
    pub fn lexical_profile(&self) -> &LanguageLexicalProfile {
        &self.lexical_profile
    }
    pub fn candidate(&self) -> &LanguageLexicalCandidate {
        &self.candidate
    }
    pub fn candidate_selection(&self) -> &LanguagePronunciationCandidateResult {
        &self.candidate_selection
    }
}
fn evaluate<I: NativeRustBinding + Clone, O: NativeRustBinding>(
    input: &I,
    hex: &str,
) -> Result<O, PronunciationSelectionRefusal> {
    use PronunciationSelectionRefusal::*;
    let program =
        conduit_plot::PortableExpressionProgram::from_canonical_hex(hex).map_err(|_| Program)?;
    let bytes = program
        .evaluate(&input.clone().encode().map_err(Native)?)
        .map_err(|_| Program)?;
    O::decode(&bytes).map_err(Native)
}
pub fn prepare_pronunciation_selection(
    lexical: &crate::lexical::PreparedLexicalTape,
    ordinal: usize,
    analysis: &LanguageAnalysisRevisionId,
    arc: &LanguageDependencyArc,
    profile: &LanguagePronunciationSelectionProfile,
) -> Result<PreparedPronunciationSelection, PronunciationSelectionRefusal> {
    use PronunciationSelectionRefusal::*;
    let tape = lexical.tape();
    let token = tape.tokens().iter().nth(ordinal).ok_or(Token)?;
    // Native laws establish exact identities. Here we additionally require both
    // arc occurrences to actually exist in this immutable reconstructed tape.
    let valid = |reference: &LanguageAnalysisTokenRef| {
        reference.revision() == analysis
            && tape
                .tokens()
                .iter()
                .any(|token| token.identity() == reference.token())
    };
    if !valid(arc.dependent())
        || !(matches!(arc.governor(), LanguageDependencyHead::Root)
            || matches!(arc.governor(), LanguageDependencyHead::Token(head)
        if head.revision() == analysis && tape.tokens().iter().any(|token| token.identity() == head.token())))
    {
        return Err(Arc);
    }
    let request = LanguagePronunciationSelectionRequest::new(
        analysis.clone(),
        arc.clone(),
        profile.clone(),
        tape.source().clone(),
        token.clone(),
    )
    .map_err(Native)?;
    let pos: LanguagePronunciationPosResult = evaluate(
        &request,
        include_str!(concat!(env!("OUT_DIR"), "/pronunciation_pos_program.hex")),
    )?;
    if !pos.accepted() {
        return Err(Policy);
    }
    let query =
        LanguagePronunciationCandidateQuery::new(request.token().candidates().clone(), *pos.pos())
            .map_err(Native)?;
    let output: LanguagePronunciationCandidateResult = evaluate(
        &query,
        include_str!(concat!(
            env!("OUT_DIR"),
            "/pronunciation_candidate_program.hex"
        )),
    )?;
    Ok(PreparedPronunciationSelection {
        request,
        lexical_profile: tape.profile().clone(),
        candidate: query
            .candidates()
            .iter()
            .nth(*output.index() as usize)
            .ok_or(Ambiguity)?
            .clone(),
        candidate_selection: output,
    })
}
pub fn pronunciation_selection_types(
) -> alloc::vec::Vec<(&'static str, conduit_core::StructuredInfoType)> {
    alloc::vec![
        (
            "LanguagePronunciationArcTarget",
            LanguagePronunciationArcTarget::semantic_type().expect("checked Type")
        ),
        (
            "LanguagePronunciationSelectionRule",
            LanguagePronunciationSelectionRule::semantic_type().expect("checked Type")
        ),
        (
            "LanguagePronunciationSelectionProfile",
            LanguagePronunciationSelectionProfile::semantic_type().expect("checked Type")
        ),
        (
            "LanguagePronunciationSelectionRequest",
            LanguagePronunciationSelectionRequest::semantic_type().expect("checked Type")
        ),
        (
            "LanguagePronunciationPosResult",
            LanguagePronunciationPosResult::semantic_type().expect("checked Type")
        ),
        (
            "LanguagePronunciationCandidateQuery",
            LanguagePronunciationCandidateQuery::semantic_type().expect("checked Type")
        ),
        (
            "LanguagePronunciationCandidateResult",
            LanguagePronunciationCandidateResult::semantic_type().expect("checked Type")
        ),
    ]
}
