//! Borrow immutable fixed-integer predicates; owned and resident checks share laws.
use super::*;

#[derive(Debug, Clone, Copy)]
pub struct FixedIntegerRange<'a> {
    pub minimum: Option<&'a [u8]>,
    pub maximum: Option<&'a [u8]>,
    pub minimum_endpoint: IntervalEndpoint,
    pub maximum_endpoint: IntervalEndpoint,
}
impl FixedIntegerRange<'_> {
    pub(super) fn validate_definition(
        &self,
        value_kind: &str,
    ) -> Result<(), ConstraintDefinitionError> {
        let kind = crate::primitive_info_kind(value_kind)
            .ok_or(ConstraintDefinitionError::WrongConstraintKind)?;
        let decode = |bytes| {
            crate::FixedInteger::decode(kind, bytes)
                .map_err(|_| ConstraintDefinitionError::WrongConstraintKind)
        };
        let minimum = self.minimum.map(decode).transpose()?;
        let maximum = self.maximum.map(decode).transpose()?;
        if super::definition::open_interval_is_invalid(
            minimum.as_ref(),
            maximum.as_ref(),
            self.minimum_endpoint,
            self.maximum_endpoint,
            |left, right| fixed_integer_cmp(*left, *right).unwrap_or(Ordering::Equal),
        ) {
            Err(ConstraintDefinitionError::InvalidFixedIntegerRange)
        } else {
            Ok(())
        }
    }
    pub(super) fn validate(
        &self,
        value_kind: &str,
        canonical: &[u8],
    ) -> Result<(), ValueConstraintRefusal> {
        let kind = crate::primitive_info_kind(value_kind)
            .ok_or(ValueConstraintRefusal::WrongConstraintKind)?;
        let value = crate::FixedInteger::decode(kind, canonical)
            .map_err(|_| ValueConstraintRefusal::FixedIntegerRange)?;
        let above = self.minimum.is_none_or(|minimum| {
            crate::FixedInteger::decode(kind, minimum).is_ok_and(|minimum| {
                fixed_integer_cmp(value, minimum)
                    .is_some_and(|order| lower_accepts(order, self.minimum_endpoint))
            })
        });
        let below = self.maximum.is_none_or(|maximum| {
            crate::FixedInteger::decode(kind, maximum).is_ok_and(|maximum| {
                fixed_integer_cmp(value, maximum)
                    .is_some_and(|order| upper_accepts(order, self.maximum_endpoint))
            })
        });
        (above && below)
            .then_some(())
            .ok_or(ValueConstraintRefusal::FixedIntegerRange)
    }
}
/// Make-owned immutable witness data. Store this once beside the emitted code.
/// Only fixed-integer range predicates are represented by this finite profile.
#[derive(Debug)]
pub struct FixedIntegerContractSpec<'a> {
    pub value_kind: &'a str,
    pub maximum_bytes: u32,
    pub range: Option<FixedIntegerRange<'a>>,
}
/// Validated view of one immutable witness, with no owned schema or bounds.
#[derive(Debug, Clone, Copy)]
pub struct BorrowedFixedIntegerContract<'a> {
    spec: &'a FixedIntegerContractSpec<'a>,
}
impl<'a> BorrowedFixedIntegerContract<'a> {
    pub fn new(spec: &'a FixedIntegerContractSpec<'a>) -> Result<Self, ConstraintDefinitionError> {
        if crate::primitive_info_kind(spec.value_kind)
            .is_none_or(|kind| crate::fixed_integer_bytes(kind) == 0)
        {
            return Err(ConstraintDefinitionError::WrongConstraintKind);
        }
        if let Some(range) = &spec.range {
            range.validate_definition(spec.value_kind)?;
        }
        Ok(Self { spec })
    }
    pub fn validate(&self, canonical: &[u8]) -> Result<(), ValueConstraintRefusal> {
        if canonical.len() > usize::try_from(self.spec.maximum_bytes).unwrap_or(usize::MAX) {
            return Err(ValueConstraintRefusal::Oversize {
                actual: u32::try_from(canonical.len()).unwrap_or(u32::MAX),
                maximum: self.spec.maximum_bytes,
            });
        }
        validate_primitive_info(self.spec.value_kind, canonical)
            .map_err(ValueConstraintRefusal::MalformedPrimitive)?;
        if let Some(range) = &self.spec.range {
            range.validate(self.spec.value_kind, canonical)?;
        }
        Ok(())
    }
    /// Finite private-work charge: size/primitive checks and the present bounds.
    pub fn validation_work(&self) -> u16 {
        2 + self.spec.range.map_or(0, |range| {
            1 + u16::from(range.minimum.is_some()) + u16::from(range.maximum.is_some())
        })
    }
}
