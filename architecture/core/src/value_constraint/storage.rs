//! Requested heap accounting, independent of constraint admission/evaluation.
use super::{CheckedValueContract, ValueConstraint};
use core::mem::size_of;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValueContractCloneStorageReservation {
    pub preparation_requested_bytes_bound: usize,
    pub retained_heap_bytes_bound: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueContractStorageRefusal {
    Capacity,
}
impl CheckedValueContract {
    /// Actual owned requested capacities, excluding this root and allocator bookkeeping.
    /// Saturation is conservative and is refused by `clone_storage_reservation`.
    pub fn owned_heap_bytes(&self) -> usize {
        contract_heap(self).unwrap_or(usize::MAX)
    }
    /// Allocation-free conservative requests for an ordinary fresh derived clone.
    /// Source spare capacities overcount cloned exact-length containers.
    pub fn clone_storage_reservation(
        &self,
    ) -> Result<ValueContractCloneStorageReservation, ValueContractStorageRefusal> {
        let bytes = contract_heap(self)?;
        Ok(ValueContractCloneStorageReservation {
            preparation_requested_bytes_bound: bytes,
            retained_heap_bytes_bound: bytes,
        })
    }
}
fn add(a: usize, b: usize) -> Result<usize, ValueContractStorageRefusal> {
    a.checked_add(b)
        .ok_or(ValueContractStorageRefusal::Capacity)
}
fn slots<T>(capacity: usize) -> Result<usize, ValueContractStorageRefusal> {
    capacity
        .checked_mul(size_of::<T>())
        .ok_or(ValueContractStorageRefusal::Capacity)
}
fn contract_heap(value: &CheckedValueContract) -> Result<usize, ValueContractStorageRefusal> {
    let mut bytes = add(
        value.value_kind.0.capacity(),
        slots::<ValueConstraint>(value.constraints.capacity())?,
    )?;
    for constraint in &value.constraints {
        let nested = match constraint {
            ValueConstraint::FixedIntegerRange {
                minimum, maximum, ..
            }
            | ValueConstraint::FloatRange {
                minimum, maximum, ..
            } => add(
                minimum.as_ref().map_or(0, |v| v.capacity()),
                maximum.as_ref().map_or(0, |v| v.capacity()),
            )?,
            ValueConstraint::CanonicalMembership { members, .. } => members
                .iter()
                .try_fold(slots::<alloc::vec::Vec<u8>>(members.capacity())?, |n, v| {
                    add(n, v.capacity())
                })?,
            ValueConstraint::TextPattern { pattern, .. } => pattern.states.iter().try_fold(
                slots::<super::TextPatternState>(pattern.states.capacity())?,
                |n, s| {
                    add(
                        n,
                        slots::<super::TextPatternTransition>(s.transitions.capacity())?,
                    )
                },
            )?,
            // Quantity is Copy { i64, unit enum }; these variants own no nested heap.
            ValueConstraint::ByteLength { .. }
            | ValueConstraint::UnsignedRange { .. }
            | ValueConstraint::SignedRange { .. }
            | ValueConstraint::QuantityRange { .. }
            | ValueConstraint::FloatFinite => 0,
        };
        bytes = add(bytes, nested)?;
    }
    Ok(bytes)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn storage_counter_overflow_refuses() {
        assert_eq!(
            add(usize::MAX, 1),
            Err(ValueContractStorageRefusal::Capacity)
        );
        assert_eq!(
            slots::<ValueConstraint>(usize::MAX),
            Err(ValueContractStorageRefusal::Capacity)
        );
    }
}
