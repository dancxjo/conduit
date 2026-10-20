//! Actual resource-backed rebase replay through the shared checked numeric body.
//! Opaque snapshots provide complete rows; Source retains committed candidates.
use crate::generated::LanguageResourceParserRebaseRequest;
use crate::{resource_lexicon_snapshot::*, *};
use alloc::vec::Vec;
use conduit_plot::{
    rust_binding::{NativeBindingRefusal, NativeRustBinding},
    PortableExpressionProgram,
};
#[derive(Debug)]
pub enum ResourceParserRebaseRefusal {
    Lineage,
    Resource,
    Revision(TextRevisionRefusal),
    Native(NativeBindingRefusal),
    Bound,
    Program,
    Evaluation(conduit_plot::PortableExpressionEvaluationRefusal),
}
pub struct PreparedResourceParserRebaseProgram {
    adapter: PortableExpressionProgram,
    numeric: PortableExpressionProgram,
}
/// Retains the exact Source request and both actual expression boundaries.
/// No observation equality or installed runtime execution is claimed.
pub struct PreparedResourceParserRebaseReplay {
    request: LanguageResourceParserRebaseRequest,
    raw: LanguageParserNumericState,
    result: LanguageParserState,
    input: Vec<u8>,
    adapted: Vec<u8>,
    output: Vec<u8>,
}
impl PreparedResourceParserRebaseProgram {
    pub fn prepare() -> Result<Self, ResourceParserRebaseRefusal> {
        use ResourceParserRebaseRefusal::Program;
        Ok(Self {
            adapter: PortableExpressionProgram::from_canonical_hex(include_str!(concat!(
                env!("OUT_DIR"),
                "/resource_parser_rebase_adapter.hex"
            )))
            .map_err(|_| Program)?,
            numeric: PortableExpressionProgram::from_canonical_hex(include_str!(concat!(
                env!("OUT_DIR"),
                "/parser_rebase_program.hex"
            )))
            .map_err(|_| Program)?,
        })
    }
    pub fn replay(
        &self,
        previous: &LanguageParserState,
        old: &PreparedResourceLexicalSnapshot<'_>,
        next: &PreparedResourceLexicalSnapshot<'_>,
        analysis: LanguageAnalysisRevisionId,
        default_relation: LanguageParserRelation,
    ) -> Result<PreparedResourceParserRebaseReplay, ResourceParserRebaseRefusal> {
        use ResourceParserRebaseRefusal::*;
        // Actual owner equality, not caller-supplied digest or membership flags.
        if !core::ptr::eq(old.lexicon(), next.lexicon()) {
            return Err(Resource);
        }
        if next.previous_tape() != Some(old.lexical().tape()) {
            return Err(Lineage);
        }
        let mut protected_end = 0;
        for dependent in 0..*previous.committed() as usize {
            let head = previous.heads()[dependent] as usize;
            for ordinal in [Some(dependent), (head < 4).then_some(head)]
                .into_iter()
                .flatten()
            {
                let token = old
                    .lexical()
                    .tape()
                    .tokens()
                    .iter()
                    .nth(ordinal)
                    .ok_or(Lineage)?;
                protected_end =
                    protected_end.max(u32::try_from(*token.span().end()).map_err(|_| Bound)?);
            }
        }
        let next_source = next.lexical().tape().source();
        validate_text_revision(
            Some(old.lexical().tape().source()),
            next_source,
            protected_end,
            4096,
        )
        .map_err(Revision)?;
        let count = next
            .lexical()
            .tape()
            .tokens()
            .iter()
            .take_while(|t| {
                matches!(t.completeness(), LanguageLexicalCompleteness::Complete)
                    && !t.candidates().is_empty()
            })
            .count() as u64;
        let previous_lexical = LanguageParserAvailableLexical::new(
            old.lexical().tape().clone(),
            *previous.token_count(),
        )
        .map_err(Native)?;
        let next_lexical =
            LanguageParserAvailableLexical::new(next.lexical().tape().clone(), count)
                .map_err(Native)?;
        let basis = LanguageParserBasis::new(
            analysis,
            next_source.material().revision().clone(),
            next_source.material().identity().clone(),
        )
        .map_err(Native)?;
        let lineage = LanguageTextRevisionLineage::new(
            next_source.clone(),
            old.lexical().tape().source().clone(),
        )
        .map_err(Native)?;
        let request = LanguageResourceParserRebaseRequest::new(
            basis,
            default_relation,
            lineage,
            next_lexical,
            previous.clone(),
            previous_lexical,
            old.lexicon().reference().clone(),
            next.lexicon().reference().clone(),
        )
        .map_err(Native)?;
        let input = request.clone().encode().map_err(Native)?;
        if input.len() > 65536 {
            return Err(Bound);
        }
        LanguageResourceParserRebaseRequest::decode(&input).map_err(Native)?;
        let adapted = self.adapter.evaluate(&input).map_err(Evaluation)?;
        if adapted.len() > 65536 {
            return Err(Bound);
        }
        let output = self.numeric.evaluate(&adapted).map_err(Evaluation)?;
        if output.len() > 65536 {
            return Err(Bound);
        }
        let raw = LanguageParserNumericState::decode(&output).map_err(Native)?;
        let result = crate::parser_rebase_replay::admit_state(&raw).map_err(Native)?;
        Ok(PreparedResourceParserRebaseReplay {
            request,
            raw,
            result,
            input,
            adapted,
            output,
        })
    }
}
impl PreparedResourceParserRebaseReplay {
    pub fn request(&self) -> &LanguageResourceParserRebaseRequest {
        &self.request
    }
    pub fn raw_result(&self) -> &LanguageParserNumericState {
        &self.raw
    }
    pub fn result(&self) -> &LanguageParserState {
        &self.result
    }
    pub fn input_canonical(&self) -> &[u8] {
        &self.input
    }
    pub fn adapted_canonical(&self) -> &[u8] {
        &self.adapted
    }
    pub fn output_canonical(&self) -> &[u8] {
        &self.output
    }
    pub fn source_id(&self) -> &'static str {
        crate::parser_commitment::COMMITMENT_SOURCE_ID
    }
}
