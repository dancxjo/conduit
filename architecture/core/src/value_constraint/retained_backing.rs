//! Actual retained allocation capacities, independent of encoded value size.
use super::*;
use core::mem::size_of;

impl CheckedValueContract {
    /// Owned heap payload only. Excludes this inline value, allocator overhead,
    /// transient validation workspace and any separately owned encoded value.
    /// Overflow refuses rather than understating the owned backing.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        let mut bytes = self.value_kind.0.capacity().checked_add(
            self.constraints
                .capacity()
                .checked_mul(size_of::<ValueConstraint>())?,
        )?;
        for constraint in &self.constraints {
            bytes = bytes.checked_add(constraint_backing(constraint)?)?;
        }
        Some(bytes)
    }
}

fn constraint_backing(value: &ValueConstraint) -> Option<usize> {
    match value {
        ValueConstraint::ByteLength { .. }
        | ValueConstraint::UnsignedRange { .. }
        | ValueConstraint::SignedRange { .. }
        | ValueConstraint::FloatFinite => Some(0),
        ValueConstraint::FixedIntegerRange {
            minimum, maximum, ..
        }
        | ValueConstraint::FloatRange {
            minimum, maximum, ..
        } => minimum
            .as_ref()
            .map_or(0, Vec::capacity)
            .checked_add(maximum.as_ref().map_or(0, Vec::capacity)),
        // Quantity is a Copy, fixed-size numerical value; its Unit is inline.
        ValueConstraint::QuantityRange {
            minimum, maximum, ..
        } => (usize::from(minimum.is_some()) + usize::from(maximum.is_some()))
            .checked_mul(size_of::<crate::Quantity>()),
        ValueConstraint::CanonicalMembership { members, .. } => {
            let mut bytes = members.capacity().checked_mul(size_of::<Vec<u8>>())?;
            for member in members {
                bytes = bytes.checked_add(member.capacity())?;
            }
            Some(bytes)
        }
        ValueConstraint::TextPattern { pattern, .. } => {
            let mut bytes = pattern
                .states
                .capacity()
                .checked_mul(size_of::<TextPatternState>())?;
            for state in &pattern.states {
                bytes = bytes.checked_add(
                    state
                        .transitions
                        .capacity()
                        .checked_mul(size_of::<TextPatternTransition>())?,
                )?;
            }
            Some(bytes)
        }
        ValueConstraint::NearQuantity {
            source_kind,
            schema,
            ..
        } => source_kind
            .0
            .capacity()
            .checked_add(schema.capacity())?
            .checked_add(size_of::<crate::Quantity>()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn counts_spare_capacity_and_nested_members_without_serializing() {
        let mut members = Vec::with_capacity(7);
        let mut member = Vec::with_capacity(123);
        member.push(1);
        let expected_member = member.capacity();
        members.push(member);
        let expected_members = members.capacity() * size_of::<Vec<u8>>();
        let mut constraints = Vec::with_capacity(5);
        constraints.push(ValueConstraint::CanonicalMembership {
            members,
            negated: false,
        });
        let value = CheckedValueContract {
            value_kind: crate::kind_id("value/bytes"),
            maximum_bytes: 128,
            constraints,
        };
        assert_eq!(
            value.retained_backing_bytes(),
            Some(
                value.value_kind.0.capacity()
                    + value.constraints.capacity() * size_of::<ValueConstraint>()
                    + expected_members
                    + expected_member
            )
        );
    }
}
