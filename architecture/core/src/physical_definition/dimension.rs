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
    /// Compose normalized dimensional algebra without heap allocation. Equal
    /// anchors combine and cancelling powers disappear before extent admission.
    pub fn product(self, other: Self) -> Result<Self, PhysicalDefinitionRefusal> {
        self.compose(other, 1)
    }
    pub fn quotient(self, other: Self) -> Result<Self, PhysicalDefinitionRefusal> {
        self.compose(other, -1)
    }
    pub fn power(self, power: i8) -> Result<Self, PhysicalDefinitionRefusal> {
        if !(-16..=16).contains(&power) {
            return Err(PhysicalDefinitionRefusal::InvalidDimensionPower);
        }
        if power == 0 {
            return Ok(Self::DIMENSIONLESS);
        }
        let mut result = self;
        for term in &mut result.terms[..result.length as usize] {
            let combined = i16::from(term.power) * i16::from(power);
            if !(-16..=16).contains(&combined) {
                return Err(PhysicalDefinitionRefusal::InvalidDimensionPower);
            }
            term.power = combined as i8;
        }
        Ok(result)
    }
    fn compose(self, other: Self, sign: i16) -> Result<Self, PhysicalDefinitionRefusal> {
        let mut result = Self::DIMENSIONLESS;
        let (mut left, mut right) = (0, 0);
        while left < self.terms().len() || right < other.terms().len() {
            let a = self.terms().get(left);
            let b = other.terms().get(right);
            let (anchor, power) = match (a, b) {
                (Some(a), Some(b)) if a.anchor == b.anchor => {
                    left += 1;
                    right += 1;
                    (a.anchor, i16::from(a.power) + sign * i16::from(b.power))
                }
                (Some(a), Some(b)) if a.anchor > b.anchor => {
                    right += 1;
                    (b.anchor, sign * i16::from(b.power))
                }
                (Some(a), _) => {
                    left += 1;
                    (a.anchor, i16::from(a.power))
                }
                (_, Some(b)) => {
                    right += 1;
                    (b.anchor, sign * i16::from(b.power))
                }
                _ => unreachable!("a remaining dimension term"),
            };
            if !(-16..=16).contains(&power) {
                return Err(PhysicalDefinitionRefusal::InvalidDimensionPower);
            }
            if power != 0 {
                if result.length as usize == MAX_DIMENSION_TERMS {
                    return Err(PhysicalDefinitionRefusal::TooManyDimensionTerms);
                }
                result.terms[result.length as usize] = DimensionTerm {
                    anchor,
                    power: power as i8,
                };
                result.length += 1;
            }
        }
        Ok(result)
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
