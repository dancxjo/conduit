//! Bounded inspection of ordinary component data outside Face.
//! This checks the original type/laws, not producer custody, current availability
//! or a rendered effect. Runtime owners must establish those separately.
use crate::parsec_input_instruction_producer::{
    ParsecInputInstructionRefusal as Refusal, PreparedLaws,
};
use crate::{
    checked_parsec_input_instruction, PARSEC_INPUT_INSTRUCTION_FRAME_BYTES,
    PARSEC_INPUT_INSTRUCTION_WORDING_BYTES,
};
use conduit_core::{validate_canonical_structured_value, ValidatedCanonicalStructuredValue};

pub struct PreparedParsecInstructionFrameInspection {
    laws: PreparedLaws,
    wording: [u8; PARSEC_INPUT_INSTRUCTION_WORDING_BYTES],
}
/// Inspected data only; this type cannot accept an input or acknowledge a Show.
pub struct InspectedParsecInstructionFrame<'a> {
    canonical: &'a [u8],
    binding: ValidatedCanonicalStructuredValue<'a>,
    wording: &'a str,
    withdrawal: bool,
}
impl InspectedParsecInstructionFrame<'_> {
    pub fn canonical_bytes(&self) -> &[u8] {
        self.canonical
    }
    pub fn binding(&self) -> ValidatedCanonicalStructuredValue<'_> {
        self.binding
    }
    pub fn wording(&self) -> &str {
        self.wording
    }
    pub fn withdrawal(&self) -> bool {
        self.withdrawal
    }
}
fn member<'a>(
    value: ValidatedCanonicalStructuredValue<'a>,
    name: &str,
) -> Result<ValidatedCanonicalStructuredValue<'a>, Refusal> {
    value
        .record_field(name)
        .map_err(|_| Refusal::Output)?
        .ok_or(Refusal::Output)
}
fn flag(value: ValidatedCanonicalStructuredValue<'_>) -> Result<bool, Refusal> {
    match value
        .primitive_bytes("value/bool")
        .map_err(|_| Refusal::Output)?
    {
        [0] => Ok(false),
        [1] => Ok(true),
        _ => Err(Refusal::Output),
    }
}
impl PreparedParsecInstructionFrameInspection {
    pub fn prepare() -> Result<Self, Refusal> {
        let owner = checked_parsec_input_instruction()?
            .native_types
            .into_iter()
            .find(|ty| ty.name == "ParsecInputInstructionInfo")
            .ok_or(Refusal::Shape)?;
        Ok(Self {
            laws: PreparedLaws::new(owner)?,
            wording: [0; PARSEC_INPUT_INSTRUCTION_WORDING_BYTES],
        })
    }
    pub fn inspect<'a>(
        &'a mut self,
        canonical: &'a [u8],
    ) -> Result<InspectedParsecInstructionFrame<'a>, Refusal> {
        if canonical.len() > PARSEC_INPUT_INSTRUCTION_FRAME_BYTES {
            return Err(Refusal::Output);
        }
        let value = validate_canonical_structured_value(canonical).map_err(|_| Refusal::Output)?;
        self.laws.validate(value).map_err(|_| Refusal::Output)?;
        if !flag(member(value, "valid")?)? {
            return Err(Refusal::Correlation);
        }
        let binding = member(value, "binding")?;
        let withdrawal = flag(member(value, "withdrawal")?)?;
        let mut length = 0usize;
        for name in ["a", "b", "c"] {
            let bytes = member(value, name)?
                .primitive_bytes(conduit_core::TEXT_INFO_ID)
                .map_err(|_| Refusal::Output)?;
            let end = length
                .checked_add(bytes.len())
                .filter(|end| *end <= self.wording.len())
                .ok_or(Refusal::Output)?;
            self.wording[length..end].copy_from_slice(bytes);
            length = end;
        }
        let wording = core::str::from_utf8(&self.wording[..length]).map_err(|_| Refusal::Output)?;
        if wording.is_empty() {
            return Err(Refusal::Output);
        }
        Ok(InspectedParsecInstructionFrame {
            canonical,
            binding,
            wording,
            withdrawal,
        })
    }
}
