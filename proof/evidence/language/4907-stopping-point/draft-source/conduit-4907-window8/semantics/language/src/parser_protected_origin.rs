//! Unwired typed custody preparation; generated Source prerequisites are required.
//! This owns complete origin evidence. It does not publish a session frontier or
//! authorize played commitment. Finite transaction storage is a separate owner.
use crate::{
    LanguageParserIndependentProtectedAdmission, LanguageParserProtectedEdgeProposal,
    LanguageParserProtectedInitialReceipt, LanguageParserProtectedInsertReceipt,
    LanguageParserProtectedOriginCorrelation, LanguageParserProtectedSetProposal,
};
use alloc::vec::Vec;
use conduit_core::StructuredInfoValue;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};
use conduit_plot::PortableExpressionProgram;

pub enum ProtectedOriginRefusal {
    Native(NativeBindingRefusal),
    Source,
    Previous,
}
impl core::fmt::Debug for ProtectedOriginRefusal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Native(v) => f.debug_tuple("Native").field(v).finish(),
            Self::Source => f.write_str("Source"),
            Self::Previous => f.write_str("Previous"),
        }
    }
}
impl From<NativeBindingRefusal> for ProtectedOriginRefusal {
    fn from(value: NativeBindingRefusal) -> Self {
        Self::Native(value)
    }
}
/// Full origin plus exact checked Source projection, with no writable fields.
pub struct PreparedProtectedOrigin {
    original: LanguageParserIndependentProtectedAdmission,
    edge: LanguageParserProtectedEdgeProposal,
    correlation: LanguageParserProtectedOriginCorrelation,
    program: PortableExpressionProgram,
    input_bytes: Vec<u8>,
    output_bytes: Vec<u8>,
}
impl PreparedProtectedOrigin {
    pub fn prepare(
        original: LanguageParserIndependentProtectedAdmission,
    ) -> Result<Self, ProtectedOriginRefusal> {
        // A compiled owned contract, never an arbitrary caller-supplied program.
        let program = PortableExpressionProgram::from_canonical_hex(include_str!(concat!(
            env!("OUT_DIR"),
            "/parser_protected_origin_edge.hex"
        )))
        .map_err(|_| ProtectedOriginRefusal::Source)?;
        let input_bytes = original
            .clone()
            .into_structured()?
            .canonical_bytes()
            .map_err(|_| ProtectedOriginRefusal::Source)?;
        let output_bytes = program
            .evaluate(&input_bytes)
            .map_err(|_| ProtectedOriginRefusal::Source)?;
        let edge = LanguageParserProtectedEdgeProposal::from_structured(
            StructuredInfoValue::from_canonical_bytes(&output_bytes)
                .map_err(|_| ProtectedOriginRefusal::Source)?,
        )?;
        let correlation =
            LanguageParserProtectedOriginCorrelation::new(original.output().clone(), edge.clone())?;
        Ok(Self {
            original,
            edge,
            correlation,
            program,
            input_bytes,
            output_bytes,
        })
    }
    pub fn original(&self) -> &LanguageParserIndependentProtectedAdmission {
        &self.original
    }
    pub fn edge(&self) -> &LanguageParserProtectedEdgeProposal {
        &self.edge
    }
    pub fn correlation(&self) -> &LanguageParserProtectedOriginCorrelation {
        &self.correlation
    }
    pub fn program(&self) -> &PortableExpressionProgram {
        &self.program
    }
    pub fn source_input(&self) -> &[u8] {
        &self.input_bytes
    }
    pub fn source_output(&self) -> &[u8] {
        &self.output_bytes
    }
    /// Source owns insert preservation. This mechanical equality ensures that
    /// the whole admitted insertion belongs to the exact currently owned set.
    pub fn insertion(
        &self,
        current: &LanguageParserProtectedSetProposal,
    ) -> Result<LanguageParserProtectedInsertReceipt, ProtectedOriginRefusal> {
        if self.original.insert().previous() != current {
            return Err(ProtectedOriginRefusal::Previous);
        }
        Ok(LanguageParserProtectedInsertReceipt::new(
            self.edge.clone(),
            self.original.output().clone(),
            current.clone(),
        )?)
    }
    /// First-origin positivity additionally requires the Source empty-set law.
    pub fn initial(&self) -> Result<LanguageParserProtectedInitialReceipt, ProtectedOriginRefusal> {
        Ok(LanguageParserProtectedInitialReceipt::new(
            self.insertion(self.original.insert().previous())?,
        )?)
    }
}
