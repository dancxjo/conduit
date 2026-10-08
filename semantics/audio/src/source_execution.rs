//! Shared bounded Source program execution evidence for Audio preparation seams.
use alloc::vec::Vec;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};
use conduit_plot::{PortableExpressionEvaluationRefusal, PortableExpressionProgram};
#[derive(Debug)]
pub enum AudioSourceExecutionRefusal {
    Admission(NativeBindingRefusal),
    InvalidProgram,
    Evaluation(PortableExpressionEvaluationRefusal),
    InvalidOutput,
}
/// Reproducible execution of one exact checked Source expression.
#[derive(Clone)]
pub struct AudioSourceExecution {
    program: &'static str,
    input: Vec<u8>,
    output: Vec<u8>,
}
impl AudioSourceExecution {
    pub fn source_program_hex(&self) -> &'static str {
        self.program
    }
    pub fn input_canonical(&self) -> &[u8] {
        &self.input
    }
    pub fn output_canonical(&self) -> &[u8] {
        &self.output
    }
}
pub(crate) struct Program {
    hex: &'static str,
    program: PortableExpressionProgram,
}
impl Program {
    pub(crate) fn new(hex: &'static str) -> Result<Self, AudioSourceExecutionRefusal> {
        Ok(Self {
            hex,
            program: PortableExpressionProgram::from_canonical_hex(hex)
                .map_err(|_| AudioSourceExecutionRefusal::InvalidProgram)?,
        })
    }
    pub(crate) fn run<T: NativeRustBinding>(
        &self,
        input: T,
        evidence: &mut Vec<AudioSourceExecution>,
    ) -> Result<Vec<u8>, AudioSourceExecutionRefusal> {
        let input = input
            .encode()
            .map_err(AudioSourceExecutionRefusal::Admission)?;
        let output = self
            .program
            .evaluate(&input)
            .map_err(AudioSourceExecutionRefusal::Evaluation)?;
        evidence.push(AudioSourceExecution {
            program: self.hex,
            input,
            output: output.clone(),
        });
        Ok(output)
    }
    pub(crate) fn native<T: NativeRustBinding, U: NativeRustBinding>(
        &self,
        input: T,
        evidence: &mut Vec<AudioSourceExecution>,
    ) -> Result<U, AudioSourceExecutionRefusal> {
        U::decode(&self.run(input, evidence)?).map_err(AudioSourceExecutionRefusal::Admission)
    }
    pub(crate) fn boolean<T: NativeRustBinding>(
        &self,
        input: T,
        evidence: &mut Vec<AudioSourceExecution>,
    ) -> Result<bool, AudioSourceExecutionRefusal> {
        match self.run(input, evidence)?.as_slice() {
            [0] => Ok(false),
            [1] => Ok(true),
            _ => Err(AudioSourceExecutionRefusal::InvalidOutput),
        }
    }
}
