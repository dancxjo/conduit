//! Mechanical revision admission; checked Source owns graph rebasing.
use crate::{lexical::*, *};
use alloc::vec::Vec;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};

pub const PARSER_REVISION_MAXIMUM_INPUT_BYTES: usize = 65_536;

pub const PARSER_REVISION_REBASE_SOURCE: &str = include_str!("../parser_revision_rebase.conduit");

#[derive(Debug)]
pub enum ParserRevisionRefusal {
    Lexical(LexicalRefusal),
    Revision(TextRevisionRefusal),
    Native(NativeBindingRefusal),
    MissingOccurrence,
    RequestBytes,
}

/// Keeps the actual prepared lexical owner beside the admitted Source request.
/// No parser action, scoring, graph mutation or commitment happens here.
pub struct PreparedParserRevisionRebase {
    request: LanguageParserRevisionRebaseRequest,
    lexical: PreparedLexicalTape,
}
impl PreparedParserRevisionRebase {
    pub fn request(&self) -> &LanguageParserRevisionRebaseRequest {
        &self.request
    }
    pub fn lexical(&self) -> &PreparedLexicalTape {
        &self.lexical
    }
    pub fn prepare(
        previous: &LanguageParserState,
        lexical: &PreparedLexicalTape,
        next: &LanguageTextRevision,
        analysis: LanguageAnalysisRevisionId,
        default_relation: LanguageParserRelation,
    ) -> Result<Self, ParserRevisionRefusal> {
        use ParserRevisionRefusal::*;
        let mut protected_end = 0;
        for dependent in 0..*previous.committed() as usize {
            let head = previous.heads()[dependent] as usize;
            for ordinal in [Some(dependent), (head < 4).then_some(head)]
                .into_iter()
                .flatten()
            {
                let token = lexical
                    .tape()
                    .tokens()
                    .iter()
                    .nth(ordinal)
                    .ok_or(MissingOccurrence)?;
                let end = u32::try_from(*token.span().end()).map_err(|_| MissingOccurrence)?;
                protected_end = protected_end.max(end);
            }
        }
        validate_text_revision(Some(lexical.tape().source()), next, protected_end, 4096)
            .map_err(Revision)?;
        let new_lexical =
            prepare_lexical_tape(next, lexical.tape().profile(), Some(lexical)).map_err(Lexical)?;
        let count = new_lexical
            .tape()
            .tokens()
            .iter()
            .take_while(|token| {
                matches!(token.completeness(), LanguageLexicalCompleteness::Complete)
                    && !token.candidates().is_empty()
            })
            .count() as u64;
        let old =
            LanguageParserAvailableLexical::new(lexical.tape().clone(), *previous.token_count())
                .map_err(Native)?;
        let new = LanguageParserAvailableLexical::new(new_lexical.tape().clone(), count)
            .map_err(Native)?;
        let basis = LanguageParserBasis::new(
            analysis,
            next.material().revision().clone(),
            next.material().identity().clone(),
        )
        .map_err(Native)?;
        let lineage =
            LanguageTextRevisionLineage::new(next.clone(), lexical.tape().source().clone())
                .map_err(Native)?;
        let request = LanguageParserRevisionRebaseRequest::new(
            basis,
            default_relation,
            lineage,
            new,
            previous.clone(),
            old,
        )
        .map_err(Native)?;
        if request.clone().encode().map_err(Native)?.len() > PARSER_REVISION_MAXIMUM_INPUT_BYTES {
            return Err(RequestBytes);
        }
        Ok(Self {
            request,
            lexical: new_lexical,
        })
    }
}

pub fn parser_revision_types() -> Vec<(&'static str, conduit_core::StructuredInfoType)> {
    alloc::vec![
        (
            "LanguageParserRevisionRebaseRequest",
            LanguageParserRevisionRebaseRequest::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserBeamCommitRequest",
            LanguageParserBeamCommitRequest::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserBeamCommitResult",
            LanguageParserBeamCommitResult::semantic_type().expect("checked Language Type")
        ),
    ]
}
