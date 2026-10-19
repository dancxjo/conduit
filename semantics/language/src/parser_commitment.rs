//! Pure preparation replay of the existing finite checked commitment chain.
//! This retains Source evidence; it is not an installed execution attestation.
use crate::{LanguageParserBeamCommitRequest, LanguageParserBeamCommitResult};
use alloc::vec::Vec;
use conduit_plot::{
    rust_binding::{NativeBindingRefusal, NativeRustBinding},
    PortableExpressionProgram,
};
include!(concat!(env!("OUT_DIR"), "/commitment_programs.rs"));
#[derive(Debug)]
pub enum ParserCommitmentRefusal {
    Native(NativeBindingRefusal),
    Program,
    Evaluation,
    Bound,
    Refused,
    Mismatch,
}
pub struct ParserCommitmentExecution {
    program: &'static str,
    input: Vec<u8>,
    output: Vec<u8>,
}
impl ParserCommitmentExecution {
    pub fn program_hex(&self) -> &'static str {
        self.program
    }
    pub fn input_canonical(&self) -> &[u8] {
        &self.input
    }
    pub fn output_canonical(&self) -> &[u8] {
        &self.output
    }
}
pub struct PreparedParserCommitment {
    request: LanguageParserBeamCommitRequest,
    result: LanguageParserBeamCommitResult,
    executions: Vec<ParserCommitmentExecution>,
}
impl PreparedParserCommitment {
    pub fn prepare(
        request: &LanguageParserBeamCommitRequest,
        expected: &LanguageParserBeamCommitResult,
    ) -> Result<Self, ParserCommitmentRefusal> {
        use ParserCommitmentRefusal::*;
        let mut value = request.clone().encode().map_err(Native)?;
        if value.len() > 65536 {
            return Err(Bound);
        }
        let admitted = LanguageParserBeamCommitRequest::decode(&value).map_err(Native)?;
        let mut executions = Vec::with_capacity(8);
        for hex in COMMITMENT_PROGRAMS {
            let program =
                PortableExpressionProgram::from_canonical_hex(hex).map_err(|_| Program)?;
            let output = program.evaluate(&value).map_err(|_| Evaluation)?;
            if output.len() > 65536 {
                return Err(Bound);
            }
            executions.push(ParserCommitmentExecution {
                program: hex,
                input: value,
                output: output.clone(),
            });
            value = output;
        }
        let result = LanguageParserBeamCommitResult::decode(&value).map_err(Native)?;
        if &result != expected {
            return Err(Mismatch);
        }
        if !*result.accepted() {
            return Err(Refused);
        }
        Ok(Self {
            request: admitted,
            result,
            executions,
        })
    }
    pub fn request(&self) -> &LanguageParserBeamCommitRequest {
        &self.request
    }
    pub fn result(&self) -> &LanguageParserBeamCommitResult {
        &self.result
    }
    pub fn executions(&self) -> &[ParserCommitmentExecution] {
        &self.executions
    }
    pub fn source_id(&self) -> &'static str {
        COMMITMENT_SOURCE_ID
    }
}
