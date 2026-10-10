use super::*;
pub const MAX_DIMENSION_TERMS: usize = 8;
pub const DIMENSION_DEFINITION_ENCODED_LEN: usize = 1 + MAX_DIMENSION_TERMS * 33;
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DimensionTerm {
    pub anchor: [u8; 32],
    pub power: i8,
}
const EMPTY: DimensionTerm = DimensionTerm {
    anchor: [0; 32],
    power: 0,
};
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DimensionDefinition {
    terms: [DimensionTerm; MAX_DIMENSION_TERMS],
    length: u8,
}
impl DimensionDefinition {
    pub(crate) const fn from_generated(bytes: &[u8], position: usize) -> Self {
        let mut value = Self::DIMENSIONLESS;
        value.length = bytes[position];
        let mut index = 0;
        while index < MAX_DIMENSION_TERMS {
            value.terms[index] = DimensionTerm {
                anchor: generated_digest(bytes, position + 1 + index * 33),
                power: bytes[position + 33 + index * 33] as i8,
            };
            index += 1;
        }
        value
    }
    pub const DIMENSIONLESS: Self = Self {
        terms: [EMPTY; MAX_DIMENSION_TERMS],
        length: 0,
    };
    pub fn new(input: &[DimensionTerm]) -> Result<Self, PhysicalDefinitionRefusal> {
        if input.len() > MAX_DIMENSION_TERMS {
            return Err(PhysicalDefinitionRefusal::TooManyDimensionTerms);
        }
        let mut result = Self::DIMENSIONLESS;
        result.terms[..input.len()].copy_from_slice(input);
        result.terms[..input.len()].sort_unstable_by_key(|term| term.anchor);
        for (index, term) in result.terms[..input.len()].iter().enumerate() {
            if term.power == 0 || !(-16..=16).contains(&term.power) {
                return Err(PhysicalDefinitionRefusal::InvalidDimensionPower);
            }
            if index > 0 && result.terms[index - 1].anchor == term.anchor {
                return Err(PhysicalDefinitionRefusal::DuplicateDimensionAnchor);
            }
        }
        result.length = input.len() as u8;
        Ok(result)
    }
    pub fn terms(&self) -> &[DimensionTerm] {
        &self.terms[..self.length as usize]
    }
    pub fn anchor(name: &str) -> Result<[u8; 32], PhysicalDefinitionRefusal> {
        let name = DefinitionName::new(name)?;
        Ok(identity(
            "physical/dimension-anchor@1",
            name.as_str().as_bytes(),
        ))
    }
    pub fn encode(self) -> [u8; DIMENSION_DEFINITION_ENCODED_LEN] {
        let mut output = [0; DIMENSION_DEFINITION_ENCODED_LEN];
        output[0] = self.length;
        for (index, term) in self.terms.iter().enumerate() {
            let start = 1 + index * 33;
            output[start..start + 32].copy_from_slice(&term.anchor);
            output[start + 32] = term.power as u8;
        }
        output
    }
    pub fn decode(input: &[u8]) -> Result<Self, PhysicalDefinitionRefusal> {
        if input.len() != DIMENSION_DEFINITION_ENCODED_LEN {
            return Err(PhysicalDefinitionRefusal::WrongEncodingLength);
        }
        let length = input[0] as usize;
        if length > MAX_DIMENSION_TERMS {
            return Err(PhysicalDefinitionRefusal::TooManyDimensionTerms);
        }
        let mut terms = [EMPTY; MAX_DIMENSION_TERMS];
        for (index, term) in terms.iter_mut().enumerate() {
            let start = 1 + index * 33;
            term.anchor.copy_from_slice(&input[start..start + 32]);
            term.power = input[start + 32] as i8;
        }
        let value = Self::new(&terms[..length])?;
        if value.encode() != input {
            return Err(PhysicalDefinitionRefusal::NonCanonicalEncoding);
        }
        Ok(value)
    }
}
