//! A narrow accepted singleton-word alignment projection of committed discourse.
//! This retains original input/output custody, not current provider authority,
//! target parser commitment, a whole preparation memory bound, or playback.
use crate::semantic::{
    TranslationProjectedDependency, TranslationSelectedWordPair,
    TranslationVocativeProjectionRequest, TranslationWordPairSelectionRequest,
};
use conduit_language::{
    committed_discourse::PreparedCommittedVocativeFact,
    discourse::{prepare_vocative_fact, DiscourseRefusal, PreparedVocativeFact},
    lexical::PreparedLexicalTape,
    LanguageDependencyArc, LanguageDependencyHead, LinguisticDerivationProvenance,
};
use conduit_plot::{
    rust_binding::{NativeBindingRefusal, NativeRustBinding},
    PortableExpressionProgram, PreparedPortableExpressionEvaluator,
};

#[derive(Debug)]
pub enum TranslationProjectionRefusal {
    SourceCustody,
    TargetLexicalBasis,
    AlignmentCustody,
    Native(NativeBindingRefusal),
    Discourse(DiscourseRefusal),
    Program,
}

/// Opaque receipt retaining the complete accepted candidate and fixed selection.
pub struct PreparedTranslationWordPair {
    request: TranslationWordPairSelectionRequest,
    program: PortableExpressionProgram,
    input: alloc::vec::Vec<u8>,
    output: alloc::vec::Vec<u8>,
    pair: TranslationSelectedWordPair,
}
impl PreparedTranslationWordPair {
    pub fn request(&self) -> &TranslationWordPairSelectionRequest {
        &self.request
    }
    pub fn pair(&self) -> &TranslationSelectedWordPair {
        &self.pair
    }
    pub fn program(&self) -> &PortableExpressionProgram {
        &self.program
    }
    pub fn input(&self) -> &[u8] {
        &self.input
    }
    pub fn output(&self) -> &[u8] {
        &self.output
    }
}
pub fn prepare_translation_word_pair(
    request: TranslationWordPairSelectionRequest,
) -> Result<PreparedTranslationWordPair, TranslationProjectionRefusal> {
    use TranslationProjectionRefusal::*;
    let program = PortableExpressionProgram::from_canonical_hex(include_str!(concat!(
        env!("OUT_DIR"),
        "/translation_select_word_pair_program.hex"
    )))
    .map_err(|_| Program)?;
    let input = request.clone().encode().map_err(Native)?;
    let output = PreparedPortableExpressionEvaluator::new(&program)
        .map_err(|_| Program)?
        .evaluate(&input)
        .map_err(|_| Program)?
        .to_vec();
    let pair = TranslationSelectedWordPair::decode(&output).map_err(Native)?;
    Ok(PreparedTranslationWordPair {
        request,
        program,
        input,
        output,
        pair,
    })
}

pub struct PreparedCommittedTranslationVocative<'a, 'commit> {
    source: &'a PreparedCommittedVocativeFact<'commit>,
    request: &'a TranslationVocativeProjectionRequest,
    target: &'a PreparedLexicalTape,
    dependent_alignment: &'a PreparedTranslationWordPair,
    governor_alignment: &'a PreparedTranslationWordPair,
    program: PortableExpressionProgram,
    input: alloc::vec::Vec<u8>,
    output: alloc::vec::Vec<u8>,
    dependency: LanguageDependencyArc,
    discourse: PreparedVocativeFact,
}

impl<'a, 'commit> PreparedCommittedTranslationVocative<'a, 'commit> {
    pub fn source(&self) -> &'a PreparedCommittedVocativeFact<'commit> {
        self.source
    }
    pub fn request(&self) -> &'a TranslationVocativeProjectionRequest {
        self.request
    }
    pub fn target(&self) -> &'a PreparedLexicalTape {
        self.target
    }
    pub fn dependent_alignment(&self) -> &PreparedTranslationWordPair {
        self.dependent_alignment
    }
    pub fn governor_alignment(&self) -> &PreparedTranslationWordPair {
        self.governor_alignment
    }
    pub fn program(&self) -> &PortableExpressionProgram {
        &self.program
    }
    pub fn input(&self) -> &[u8] {
        &self.input
    }
    pub fn output(&self) -> &[u8] {
        &self.output
    }
    pub fn dependency(&self) -> &LanguageDependencyArc {
        &self.dependency
    }
    pub fn discourse(&self) -> &PreparedVocativeFact {
        &self.discourse
    }
}

pub fn prepare_committed_translation_vocative<'a, 'commit>(
    source: &'a PreparedCommittedVocativeFact<'commit>,
    request: &'a TranslationVocativeProjectionRequest,
    target: &'a PreparedLexicalTape,
    dependent_alignment: &'a PreparedTranslationWordPair,
    governor_alignment: &'a PreparedTranslationWordPair,
    identity: alloc::string::String,
    provenance: LinguisticDerivationProvenance,
) -> Result<PreparedCommittedTranslationVocative<'a, 'commit>, TranslationProjectionRefusal> {
    use TranslationProjectionRefusal::*;
    if dependent_alignment.request().candidate() != governor_alignment.request().candidate()
        || dependent_alignment.request().accepted() != governor_alignment.request().accepted()
        || request.dependent_alignment() != dependent_alignment.pair()
        || request.governor_alignment() != governor_alignment.pair()
    {
        return Err(AlignmentCustody);
    }
    if request.source() != source.fact().fact() {
        return Err(SourceCustody);
    }
    let original = source
        .committed()
        .admission()
        .fact()
        .query()
        .beam()
        .lexical()
        .tape();
    let tokens = original.tokens().as_slice();
    let dependent = usize::try_from(*request.source().basis().dependent().token().ordinal())
        .map_err(|_| SourceCustody)?;
    let governor =
        usize::try_from(*request.source_governor().token().ordinal()).map_err(|_| SourceCustody)?;
    if tokens.get(dependent) != Some(request.source_dependent_token())
        || tokens.get(governor) != Some(request.source_governor_token())
    {
        return Err(SourceCustody);
    }
    if request.target() != target.tape() {
        return Err(TargetLexicalBasis);
    }
    let program = PortableExpressionProgram::from_canonical_hex(include_str!(concat!(
        env!("OUT_DIR"),
        "/translation_project_vocative_program.hex"
    )))
    .map_err(|_| Program)?;
    let input = request.clone().encode().map_err(Native)?;
    let output = PreparedPortableExpressionEvaluator::new(&program)
        .map_err(|_| Program)?
        .evaluate(&input)
        .map_err(|_| Program)?
        .to_vec();
    let raw = TranslationProjectedDependency::decode(&output).map_err(Native)?;
    let dependency = LanguageDependencyArc::new(
        raw.dependent().clone(),
        LanguageDependencyHead::token(
            raw.governor().revision().clone(),
            raw.governor().token().clone(),
        )
        .map_err(Native)?,
        raw.relation().clone(),
    )
    .map_err(Native)?;
    let discourse = prepare_vocative_fact(
        identity,
        target.tape().source(),
        dependency.dependent().revision(),
        &dependency,
        provenance,
        target.tape().tokens().len() as u64,
    )
    .map_err(Discourse)?;
    Ok(PreparedCommittedTranslationVocative {
        source,
        request,
        target,
        dependent_alignment,
        governor_alignment,
        program,
        input,
        output,
        dependency,
        discourse,
    })
}
