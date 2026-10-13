//! Consumer numerical requirements. Mathematical meaning and measurement
//! uncertainty are independent of a selected execution representation.
use crate::numeric_realization::Binary64Rounding;
use crate::{MathValue, NumericRefusal, MAX_INTEGER_LIMBS, MAX_SYMBOLIC_NODES};
use alloc::vec::Vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericRepresentation {
    CheckedI64,
    ExactRational,
    Symbolic,
    CertifiedInterval,
    Binary64,
    IeeeBinary64,
    /// Planner chooses an eligible offered algorithm before considering cost.
    Automatic,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericSemantics {
    Mathematical,
    IeeeOperational,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NumericAccuracy {
    Exact,
    Absolute(MathValue),
    Relative {
        relative: MathValue,
        zero_absolute: MathValue,
    },
    Ulps(u32),
    EnclosureWidth(MathValue),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumericExecutionBudget {
    pub storage_bytes: u32,
    /// Reviewed primitive-call units, each with its own finite limb-work guard.
    /// These are neither processor instructions nor a latency measurement.
    pub work_steps: u64,
    pub limbs: u16,
    pub symbolic_nodes: u16,
}
pub const NUMERIC_WORK_UNIT_ID: &str = "numeric/bounded-primitive-call@1";
impl NumericExecutionBudget {
    pub fn validate(&self) -> Result<(), NumericRefusal> {
        if self.storage_bytes == 0 || self.storage_bytes > 1_048_576 {
            return Err(NumericRefusal::StorageBudget);
        }
        if self.work_steps == 0 || self.work_steps > 10_000_000 {
            return Err(NumericRefusal::WorkBudget);
        }
        if self.limbs == 0 || usize::from(self.limbs) > MAX_INTEGER_LIMBS {
            return Err(NumericRefusal::IntegerBudget);
        }
        if usize::from(self.symbolic_nodes) > MAX_SYMBOLIC_NODES {
            return Err(NumericRefusal::NodeBudget);
        }
        Ok(())
    }
}
/// Permitted operational exceptions. Flags are always retained as evidence.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NumericExceptionPolicy {
    pub invalid: bool,
    pub division_by_zero: bool,
    pub overflow: bool,
    pub underflow: bool,
    pub inexact: bool,
}
impl NumericExceptionPolicy {
    pub fn permits(self, flags: crate::numeric_realization::Binary64Flags) -> bool {
        (!flags.invalid || self.invalid)
            && (!flags.division_by_zero || self.division_by_zero)
            && (!flags.overflow || self.overflow)
            && (!flags.underflow || self.underflow)
            && (!flags.inexact || self.inexact)
    }
}
/// A uniform preparation-time range assumption, not an observed sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumericParameterRange {
    pub slot: u16,
    pub lower: MathValue,
    pub upper: MathValue,
}
impl NumericParameterRange {
    pub fn validate(&self) -> Result<(), NumericRefusal> {
        match self.lower.compare(&self.upper)? {
            Some(core::cmp::Ordering::Less | core::cmp::Ordering::Equal) => Ok(()),
            Some(core::cmp::Ordering::Greater) => Err(NumericRefusal::Domain),
            None => Err(NumericRefusal::IndeterminateDomain),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumericRequirement {
    pub semantics: NumericSemantics,
    pub representation: NumericRepresentation,
    pub rounding: Binary64Rounding,
    pub accuracy: NumericAccuracy,
    pub parameters: Vec<NumericParameterRange>,
    pub budget: NumericExecutionBudget,
    /// Ordered operations and rounding must reproduce the admitted bit result.
    pub reproducible: bool,
    pub exceptions: NumericExceptionPolicy,
}
fn nonnegative(value: &MathValue) -> Result<(), NumericRefusal> {
    match value.compare(&MathValue::integer(0))? {
        Some(core::cmp::Ordering::Greater | core::cmp::Ordering::Equal) => Ok(()),
        Some(core::cmp::Ordering::Less) => Err(NumericRefusal::Domain),
        None => Err(NumericRefusal::IndeterminateDomain),
    }
}
impl NumericRequirement {
    pub fn validate(&self) -> Result<(), NumericRefusal> {
        self.budget.validate()?;
        if self.semantics == NumericSemantics::IeeeOperational
            && !matches!(self.accuracy, NumericAccuracy::Exact)
        {
            return Err(NumericRefusal::InvalidProfile);
        }
        if self.representation == NumericRepresentation::Symbolic && self.budget.symbolic_nodes == 0
        {
            return Err(NumericRefusal::NodeBudget);
        }
        if self.parameters.len() > 64 {
            return Err(NumericRefusal::StorageBudget);
        }
        for (index, range) in self.parameters.iter().enumerate() {
            range.validate()?;
            if index > 0 && self.parameters[index - 1].slot >= range.slot {
                return Err(NumericRefusal::NonCanonical);
            }
        }
        if (self.semantics == NumericSemantics::IeeeOperational)
            != (self.representation == NumericRepresentation::IeeeBinary64)
        {
            return Err(NumericRefusal::InvalidProfile);
        }
        match &self.accuracy {
            NumericAccuracy::Exact => {}
            NumericAccuracy::Absolute(value) | NumericAccuracy::EnclosureWidth(value) => {
                nonnegative(value)?
            }
            NumericAccuracy::Relative {
                relative,
                zero_absolute,
            } => {
                nonnegative(relative)?;
                nonnegative(zero_absolute)?;
            }
            NumericAccuracy::Ulps(_)
                if matches!(
                    self.representation,
                    NumericRepresentation::Binary64 | NumericRepresentation::IeeeBinary64
                ) => {}
            NumericAccuracy::Ulps(_) => return Err(NumericRefusal::InvalidProfile),
        }
        Ok(())
    }
}
/// This remains source evidence and never pays a numerical error allowance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumericMeasurementUncertainty {
    pub lower: MathValue,
    pub upper: MathValue,
}
impl NumericMeasurementUncertainty {
    pub fn validate(&self) -> Result<(), NumericRefusal> {
        NumericParameterRange {
            slot: 0,
            lower: self.lower.clone(),
            upper: self.upper.clone(),
        }
        .validate()
    }
}

pub const MAX_NUMERIC_REQUIREMENT_BYTES: usize = 65_536;
pub const NUMERIC_REQUIREMENT_INFO_ID: &str = "numeric/consumer-requirement@1";
pub const NUMERIC_REALIZATION_PROPERTY_ID: &str = "numeric/verified-realization@1";
mod configuration;
pub use configuration::{numeric_requirement_info_type, numeric_requirement_profile};
mod report;
pub use report::*;
mod offer;
pub use offer::*;
impl NumericRequirement {
    /// Canonical consumer contract, retained in Plan identity and certificates.
    /// Encoding is an owned preparation operation; it is never an implicit Play
    /// allocation or a claim that an algorithm meets the requested tolerance.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, NumericRefusal> {
        self.validate()?;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&[
            1,
            self.semantics as u8,
            self.representation as u8,
            self.rounding as u8,
            u8::from(self.reproducible),
        ]);
        match &self.accuracy {
            NumericAccuracy::Exact => bytes.push(0),
            NumericAccuracy::Absolute(value) => {
                bytes.push(1);
                append_value(&mut bytes, value)?;
            }
            NumericAccuracy::Relative {
                relative,
                zero_absolute,
            } => {
                bytes.push(2);
                append_value(&mut bytes, relative)?;
                append_value(&mut bytes, zero_absolute)?;
            }
            NumericAccuracy::Ulps(value) => {
                bytes.push(3);
                bytes.extend_from_slice(&value.to_le_bytes());
            }
            NumericAccuracy::EnclosureWidth(value) => {
                bytes.push(4);
                append_value(&mut bytes, value)?;
            }
        }
        for flag in [
            self.exceptions.invalid,
            self.exceptions.division_by_zero,
            self.exceptions.overflow,
            self.exceptions.underflow,
            self.exceptions.inexact,
        ] {
            bytes.push(u8::from(flag));
        }
        bytes.extend_from_slice(&self.budget.storage_bytes.to_le_bytes());
        bytes.extend_from_slice(&self.budget.work_steps.to_le_bytes());
        bytes.extend_from_slice(&self.budget.limbs.to_le_bytes());
        bytes.extend_from_slice(&self.budget.symbolic_nodes.to_le_bytes());
        bytes.extend_from_slice(&(self.parameters.len() as u16).to_le_bytes());
        for range in &self.parameters {
            bytes.extend_from_slice(&range.slot.to_le_bytes());
            append_value(&mut bytes, &range.lower)?;
            append_value(&mut bytes, &range.upper)?;
        }
        if bytes.len() > MAX_NUMERIC_REQUIREMENT_BYTES {
            return Err(NumericRefusal::StorageBudget);
        }
        Ok(bytes)
    }
    pub fn identity(&self) -> Result<[u8; 32], NumericRefusal> {
        Ok(crate::semantic_digest(
            NUMERIC_REQUIREMENT_INFO_ID,
            &self.canonical_bytes()?,
        ))
    }
}
fn append_value(bytes: &mut Vec<u8>, value: &MathValue) -> Result<(), NumericRefusal> {
    let canonical = value.canonical_bytes()?;
    if bytes
        .len()
        .checked_add(4 + canonical.len())
        .is_none_or(|length| length > MAX_NUMERIC_REQUIREMENT_BYTES)
    {
        return Err(NumericRefusal::StorageBudget);
    }
    bytes.extend_from_slice(&(canonical.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&canonical);
    Ok(())
}

impl NumericRequirement {
    /// Checked canonical custody for stored Plans. No field is trusted merely
    /// because it arrived in an already parsed structured record.
    pub fn decode(bytes: &[u8]) -> Result<Self, NumericRefusal> {
        if bytes.len() > MAX_NUMERIC_REQUIREMENT_BYTES {
            return Err(NumericRefusal::StorageBudget);
        }
        let mut cursor = RequirementCursor { bytes, offset: 0 };
        if cursor.byte()? != 1 {
            return Err(NumericRefusal::UnsupportedVersion);
        }
        let semantics = match cursor.byte()? {
            0 => NumericSemantics::Mathematical,
            1 => NumericSemantics::IeeeOperational,
            _ => return Err(NumericRefusal::MalformedEncoding),
        };
        let representation = match cursor.byte()? {
            0 => NumericRepresentation::CheckedI64,
            1 => NumericRepresentation::ExactRational,
            2 => NumericRepresentation::Symbolic,
            3 => NumericRepresentation::CertifiedInterval,
            4 => NumericRepresentation::Binary64,
            5 => NumericRepresentation::IeeeBinary64,
            6 => NumericRepresentation::Automatic,
            _ => return Err(NumericRefusal::MalformedEncoding),
        };
        let rounding = match cursor.byte()? {
            0 => Binary64Rounding::NearestEven,
            1 => Binary64Rounding::NearestAway,
            2 => Binary64Rounding::TowardZero,
            3 => Binary64Rounding::TowardPositive,
            4 => Binary64Rounding::TowardNegative,
            _ => return Err(NumericRefusal::MalformedEncoding),
        };
        let reproducible = cursor.boolean()?;
        let accuracy = match cursor.byte()? {
            0 => NumericAccuracy::Exact,
            1 => NumericAccuracy::Absolute(cursor.value()?),
            2 => NumericAccuracy::Relative {
                relative: cursor.value()?,
                zero_absolute: cursor.value()?,
            },
            3 => NumericAccuracy::Ulps(u32::from_le_bytes(cursor.array()?)),
            4 => NumericAccuracy::EnclosureWidth(cursor.value()?),
            _ => return Err(NumericRefusal::MalformedEncoding),
        };
        let exceptions = NumericExceptionPolicy {
            invalid: cursor.boolean()?,
            division_by_zero: cursor.boolean()?,
            overflow: cursor.boolean()?,
            underflow: cursor.boolean()?,
            inexact: cursor.boolean()?,
        };
        let budget = NumericExecutionBudget {
            storage_bytes: u32::from_le_bytes(cursor.array()?),
            work_steps: u64::from_le_bytes(cursor.array()?),
            limbs: u16::from_le_bytes(cursor.array()?),
            symbolic_nodes: u16::from_le_bytes(cursor.array()?),
        };
        let count = usize::from(u16::from_le_bytes(cursor.array()?));
        if count > 64 {
            return Err(NumericRefusal::StorageBudget);
        }
        let mut parameters = Vec::with_capacity(count);
        for _ in 0..count {
            parameters.push(NumericParameterRange {
                slot: u16::from_le_bytes(cursor.array()?),
                lower: cursor.value()?,
                upper: cursor.value()?,
            });
        }
        if cursor.offset != bytes.len() {
            return Err(NumericRefusal::NonCanonical);
        }
        let value = Self {
            semantics,
            representation,
            rounding,
            accuracy,
            parameters,
            budget,
            reproducible,
            exceptions,
        };
        value.validate()?;
        Ok(value)
    }
}
struct RequirementCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl RequirementCursor<'_> {
    fn take(&mut self, length: usize) -> Result<&[u8], NumericRefusal> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(NumericRefusal::MalformedEncoding)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(NumericRefusal::MalformedEncoding)?;
        self.offset = end;
        Ok(value)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], NumericRefusal> {
        self.take(N)?
            .try_into()
            .map_err(|_| NumericRefusal::MalformedEncoding)
    }
    fn byte(&mut self) -> Result<u8, NumericRefusal> {
        Ok(self.array::<1>()?[0])
    }
    fn boolean(&mut self) -> Result<bool, NumericRefusal> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(NumericRefusal::NonCanonical),
        }
    }
    fn value(&mut self) -> Result<MathValue, NumericRefusal> {
        let length = u32::from_le_bytes(self.array()?) as usize;
        MathValue::decode(self.take(length)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn requirement() -> NumericRequirement {
        NumericRequirement {
            semantics: NumericSemantics::Mathematical,
            representation: NumericRepresentation::Binary64,
            rounding: Binary64Rounding::NearestEven,
            accuracy: NumericAccuracy::Absolute(MathValue::parse_decimal("0.01").unwrap()),
            parameters: Vec::new(),
            budget: NumericExecutionBudget {
                storage_bytes: 4096,
                work_steps: 100_000,
                limbs: 128,
                symbolic_nodes: 128,
            },
            reproducible: true,
            exceptions: NumericExceptionPolicy {
                inexact: true,
                ..NumericExceptionPolicy::default()
            },
        }
    }
    #[test]
    fn tolerance_and_uniform_range_are_checked_before_selection() {
        let mut request = requirement();
        request.validate().unwrap();
        request.accuracy = NumericAccuracy::Absolute(MathValue::integer(-1));
        assert_eq!(request.validate(), Err(NumericRefusal::Domain));
        request.accuracy = NumericAccuracy::Exact;
        request.parameters.push(NumericParameterRange {
            slot: 0,
            lower: MathValue::integer(2),
            upper: MathValue::integer(1),
        });
        assert_eq!(request.validate(), Err(NumericRefusal::Domain));
        request.parameters[0].upper = MathValue::integer(2);
        request.validate().unwrap();
        request.parameters.push(request.parameters[0].clone());
        assert_eq!(request.validate(), Err(NumericRefusal::NonCanonical));
    }
    #[test]
    fn canonical_requirement_custody_preserves_rounding_and_refuses_tampering() {
        let request = requirement();
        assert_eq!(
            NumericRequirement::from_configuration(&request.configuration().unwrap()).unwrap(),
            request
        );
        let bytes = request.canonical_bytes().unwrap();
        assert_eq!(NumericRequirement::decode(&bytes).unwrap(), request);
        let mut different = request.clone();
        different.rounding = Binary64Rounding::TowardZero;
        assert_ne!(request.identity().unwrap(), different.identity().unwrap());
        let mut forged = bytes.clone();
        forged[4] = 2; // Noncanonical reproducibility Boolean.
        assert_eq!(
            NumericRequirement::decode(&forged),
            Err(NumericRefusal::NonCanonical)
        );
        assert!(NumericRequirement::decode(&bytes[..bytes.len() - 1]).is_err());
        let mut trailing = bytes;
        trailing.push(0);
        assert_eq!(
            NumericRequirement::decode(&trailing),
            Err(NumericRefusal::NonCanonical)
        );
    }
    #[test]
    fn ieee_mode_and_machine_representation_must_agree() {
        let mut request = requirement();
        request.semantics = NumericSemantics::IeeeOperational;
        assert_eq!(request.validate(), Err(NumericRefusal::InvalidProfile));
        request.representation = NumericRepresentation::IeeeBinary64;
        request.accuracy = NumericAccuracy::Exact;
        request.validate().unwrap();
        request.budget.work_steps = 0;
        assert_eq!(request.validate(), Err(NumericRefusal::WorkBudget));
    }
}
