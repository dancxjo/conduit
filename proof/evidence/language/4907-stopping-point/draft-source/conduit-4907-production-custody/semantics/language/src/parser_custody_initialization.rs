//! Private preparation of the exact Source-owned empty protection initializer.
//! This receipt initializes custody; it does not authorize a parser beam,
//! dependency commitment, or played audio. Not wired into the public Session.
use crate::{
    LanguageParserProtectedEdgeProposal, LanguageParserProtectedInitializationReceipt,
    LanguageParserProtectedSetProposal,
};
use alloc::vec::Vec;
use conduit_core::StructuredInfoValue;
use conduit_plot::{
    rust_binding::{NativeBindingRefusal, NativeRustBinding},
    PortableExpressionProgram,
};

#[derive(Debug)]
pub(crate) enum InitializationRefusal {
    Native(NativeBindingRefusal),
    Source,
}
impl From<NativeBindingRefusal> for InitializationRefusal {
    fn from(value: NativeBindingRefusal) -> Self {
        Self::Native(value)
    }
}
/// Retains the full fixed program and original execution bytes, not a caller's
/// assertion that a well-formed Native receipt was produced by that program.
pub(crate) struct PreparedCustodyInitialization {
    receipt: LanguageParserProtectedInitializationReceipt,
    program: PortableExpressionProgram,
    input: Vec<u8>,
    output: Vec<u8>,
}
impl PreparedCustodyInitialization {
    pub(crate) fn prepare(
        input: LanguageParserProtectedEdgeProposal,
    ) -> Result<Self, InitializationRefusal> {
        let program = PortableExpressionProgram::from_canonical_hex(include_str!(concat!(
            env!("OUT_DIR"),
            "/parser_protected_set_initialize.hex"
        )))
        .map_err(|_| InitializationRefusal::Source)?;
        let encoded = input.clone().encode()?;
        let output = program
            .evaluate(&encoded)
            .map_err(|_| InitializationRefusal::Source)?;
        let value = StructuredInfoValue::from_canonical_bytes(&output)
            .map_err(|_| InitializationRefusal::Source)?;
        let protection = LanguageParserProtectedSetProposal::from_structured(value)?;
        // The checked receipt owns sentinel and whole empty-set correlation laws.
        let receipt = LanguageParserProtectedInitializationReceipt::new(input, protection)?;
        Ok(Self {
            receipt,
            program,
            input: encoded,
            output,
        })
    }
    pub(crate) fn receipt(&self) -> &LanguageParserProtectedInitializationReceipt {
        &self.receipt
    }
    pub(crate) fn program(&self) -> &PortableExpressionProgram {
        &self.program
    }
    pub(crate) fn source_input(&self) -> &[u8] {
        &self.input
    }
    pub(crate) fn source_output(&self) -> &[u8] {
        &self.output
    }
}
