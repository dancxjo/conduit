//! Exact initial Source replay; supplied observations are not installed proof.
use crate::{generated::LanguageAnalysisInitialRawBeam, lexical::PreparedLexicalTape, *};
use alloc::vec::Vec;
use conduit_plot::{
    rust_binding::{NativeBindingRefusal, NativeRustBinding},
    PortableExpressionProgram,
};
#[derive(Debug)]
pub enum AnalysisInitializationRefusal {
    Native(NativeBindingRefusal),
    Program,
    Evaluation,
    ObservationMismatch,
    LexicalBasis,
    Bound,
}
pub struct PreparedAnalysisInitializationProgram {
    initialize: PortableExpressionProgram,
    envelope: PortableExpressionProgram,
}
pub struct PreparedAnalysisInitialization {
    pub(crate) lexical: PreparedLexicalTape,
    pub(crate) beam: LanguageParserScorerBeam,
    request: LanguageParserBegin,
    input: Vec<u8>,
    output: Vec<u8>,
    envelope_output: Vec<u8>,
}
impl PreparedAnalysisInitializationProgram {
    pub fn prepare() -> Result<Self, AnalysisInitializationRefusal> {
        use AnalysisInitializationRefusal::*;
        Ok(Self {
            initialize: PortableExpressionProgram::from_canonical_hex(include_str!(concat!(
                env!("OUT_DIR"),
                "/analysis_initialize_program.hex"
            )))
            .map_err(|_| Program)?,
            envelope: PortableExpressionProgram::from_canonical_hex(include_str!(concat!(
                env!("OUT_DIR"),
                "/analysis_initial_envelope.hex"
            )))
            .map_err(|_| Program)?,
        })
    }
    pub fn replay(
        &self,
        lexical: PreparedLexicalTape,
        request: &LanguageParserBegin,
        observed: &LanguageParserNumericState,
    ) -> Result<PreparedAnalysisInitialization, AnalysisInitializationRefusal> {
        use AnalysisInitializationRefusal::*;
        let tape = lexical.tape();
        if tape.tokens().is_empty() || tape.tokens().len() > 4 {
            return Err(LexicalBasis);
        }
        let available = tape
            .tokens()
            .iter()
            .take_while(|t| {
                matches!(t.completeness(), LanguageLexicalCompleteness::Complete)
                    && !t.candidates().is_empty()
            })
            .count() as u64;
        if request.basis().text() != tape.source().material().identity()
            || request.basis().source_revision() != tape.source().material().revision()
            || *request.token_count() != available
        {
            return Err(LexicalBasis);
        }
        let input = request.clone().encode().map_err(Native)?;
        if input.len() > 65536 {
            return Err(Bound);
        }
        LanguageParserBegin::decode(&input).map_err(Native)?;
        let output = self.initialize.evaluate(&input).map_err(|_| Evaluation)?;
        if output.len() > 65536 {
            return Err(Bound);
        }
        let actual = LanguageParserNumericState::decode(&output).map_err(Native)?;
        if &actual != observed {
            return Err(ObservationMismatch);
        }
        let envelope_output = self.envelope.evaluate(&output).map_err(|_| Evaluation)?;
        if envelope_output.len() > 65536 {
            return Err(Bound);
        }
        let raw = LanguageAnalysisInitialRawBeam::decode(&envelope_output).map_err(Native)?;
        let beam = LanguageParserScorerBeam::new(
            admit(raw.candidate0())?,
            admit(raw.candidate1())?,
            admit(raw.candidate2())?,
            admit(raw.candidate3())?,
        )
        .map_err(Native)?;
        Ok(PreparedAnalysisInitialization {
            lexical,
            beam,
            request: request.clone(),
            input,
            output,
            envelope_output,
        })
    }
}
pub(crate) fn admit(
    raw: &LanguageParserScorerRawHypothesis,
) -> Result<LanguageParserScorerHypothesis, AnalysisInitializationRefusal> {
    use AnalysisInitializationRefusal::Native;
    let parser = raw.parser();
    let state = parser.state();
    let state = LanguageParserState::new(
        state.basis().clone(),
        *state.committed(),
        *state.depth(),
        *state.heads(),
        state.relation0().clone(),
        state.relation1().clone(),
        state.relation2().clone(),
        state.relation3().clone(),
        *state.stack(),
        *state.token_count(),
        *state.unread(),
    )
    .map_err(Native)?;
    let parser =
        LanguageParserHypothesis::new(*parser.active(), *parser.identity(), *parser.score(), state)
            .map_err(Native)?;
    LanguageParserScorerHypothesis::new(*raw.choices(), parser, *raw.selected()).map_err(Native)
}
impl PreparedAnalysisInitialization {
    pub fn request(&self) -> &LanguageParserBegin {
        &self.request
    }
    pub fn beam(&self) -> &LanguageParserScorerBeam {
        &self.beam
    }
    pub fn input_canonical(&self) -> &[u8] {
        &self.input
    }
    pub fn output_canonical(&self) -> &[u8] {
        &self.output
    }
    pub fn envelope_output_canonical(&self) -> &[u8] {
        &self.envelope_output
    }
    pub fn source_id(&self) -> &'static str {
        crate::parser_commitment::COMMITMENT_SOURCE_ID
    }
    pub fn initializer_program_hex(&self) -> &'static str {
        include_str!(concat!(env!("OUT_DIR"), "/analysis_initialize_program.hex"))
    }
    pub fn envelope_program_hex(&self) -> &'static str {
        include_str!(concat!(env!("OUT_DIR"), "/analysis_initial_envelope.hex"))
    }
}

pub fn analysis_epoch_types() -> Vec<(&'static str, conduit_core::StructuredInfoType)> {
    alloc::vec![
        (
            "LanguageAnalysisInitialRawBeam",
            LanguageAnalysisInitialRawBeam::semantic_type().expect("checked raw envelope")
        ),
        (
            "LanguageAnalysisRebasedEnvelopeInput",
            crate::generated::LanguageAnalysisRebasedEnvelopeInput::semantic_type()
                .expect("checked rebase envelope")
        ),
    ]
}
