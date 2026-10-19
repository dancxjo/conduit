//! Replay of one existing checked rebase expression, not installed currentness.
use crate::{parser_revision::*, *};
use alloc::vec::Vec;
use conduit_plot::{
    rust_binding::{NativeBindingRefusal, NativeRustBinding},
    PortableExpressionProgram,
};
const PROGRAM: &str = include_str!(concat!(env!("OUT_DIR"), "/parser_rebase_program.hex"));
#[derive(Debug)]
pub enum ParserRebaseReplayRefusal {
    Native(NativeBindingRefusal),
    Program,
    Evaluation,
    Bound,
    ObservationMismatch,
}
/// Owns lexical preparation and exact request/output custody for one survivor.
/// Neither a serialized observation nor this replay grants installed authority.
pub struct PreparedParserRebaseReplay {
    preparation: PreparedParserRevisionRebase,
    raw_result: LanguageParserNumericState,
    result: LanguageParserState,
    input: Vec<u8>,
    output: Vec<u8>,
}
/// Immutable Source program preparation shared across survivors and epochs.
pub struct PreparedParserRebaseProgram {
    program: PortableExpressionProgram,
}
impl PreparedParserRebaseProgram {
    pub fn prepare() -> Result<Self, ParserRebaseReplayRefusal> {
        Ok(Self {
            program: PortableExpressionProgram::from_canonical_hex(PROGRAM)
                .map_err(|_| ParserRebaseReplayRefusal::Program)?,
        })
    }
    pub fn replay(
        &self,
        preparation: PreparedParserRevisionRebase,
        observed: &LanguageParserNumericState,
    ) -> Result<PreparedParserRebaseReplay, ParserRebaseReplayRefusal> {
        use ParserRebaseReplayRefusal::*;
        let input = preparation.request().clone().encode().map_err(Native)?;
        if input.len() > PARSER_REVISION_MAXIMUM_INPUT_BYTES {
            return Err(Bound);
        }
        // Decode executes every generated request constraint, including nested
        // Source laws, before the retained expression consumes its input.
        LanguageParserRevisionRebaseRequest::decode(&input).map_err(Native)?;
        let output = self.program.evaluate(&input).map_err(|_| Evaluation)?;
        if output.len() > PARSER_REVISION_MAXIMUM_INPUT_BYTES {
            return Err(Bound);
        }
        let raw = LanguageParserNumericState::decode(&output).map_err(Native)?;
        if &raw != observed {
            return Err(ObservationMismatch);
        }
        // Exact representation adapter only: Source already chose all fields.
        let result = LanguageParserState::new(
            raw.basis().clone(),
            *raw.committed(),
            *raw.depth(),
            *raw.heads(),
            raw.relation0().clone(),
            raw.relation1().clone(),
            raw.relation2().clone(),
            raw.relation3().clone(),
            *raw.stack(),
            *raw.token_count(),
            *raw.unread(),
        )
        .map_err(Native)?;
        Ok(PreparedParserRebaseReplay {
            preparation,
            raw_result: raw,
            result,
            input,
            output,
        })
    }
}
impl PreparedParserRebaseReplay {
    pub fn preparation(&self) -> &PreparedParserRevisionRebase {
        &self.preparation
    }
    pub fn raw_result(&self) -> &LanguageParserNumericState {
        &self.raw_result
    }
    pub fn result(&self) -> &LanguageParserState {
        &self.result
    }
    pub fn input_canonical(&self) -> &[u8] {
        &self.input
    }
    pub fn output_canonical(&self) -> &[u8] {
        &self.output
    }
    pub fn program_hex(&self) -> &'static str {
        PROGRAM
    }
    pub fn source_id(&self) -> &'static str {
        crate::parser_commitment::COMMITMENT_SOURCE_ID
    }
}
