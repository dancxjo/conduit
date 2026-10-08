//! Requested storage for the compatibility traversal only. Core admission stays
//! authoritative. Root structs, allocator bookkeeping and stack are separate.
use super::*;
use core::mem::size_of;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraversalStorageReceipt {
    pub preparation_requested_bytes_bound: usize,
    pub retained_heap_bytes_bound: usize,
    pub retained_heap_bytes: usize,
}
#[derive(Debug)]
pub enum TraversalStorageRefusal {
    Capacity,
    CoreStorage(conduit_core::PreparedStructuredValidationStorageRefusal),
    ContractStorage(conduit_core::ValueContractStorageRefusal),
    Structure(Refusal),
    Contract(StructuredContractValidationRefusal),
}
fn add(a: usize, b: usize) -> Result<usize, TraversalStorageRefusal> {
    a.checked_add(b).ok_or(TraversalStorageRefusal::Capacity)
}
fn array<T>(n: usize) -> Result<usize, TraversalStorageRefusal> {
    n.checked_mul(size_of::<T>())
        .ok_or(TraversalStorageRefusal::Capacity)
}
impl PreparedStructuredValueValidator {
    pub fn owned_heap_bytes(&self) -> usize {
        self.inner
            .owned_heap_bytes()
            .saturating_add(self.value_type.owned_heap_bytes())
            .saturating_add(self.prefix.capacity())
    }
    pub fn storage_reservation(
        ty: &StructuredInfoType,
        maximum: usize,
    ) -> Result<TraversalStorageReceipt, TraversalStorageRefusal> {
        let inner =
            conduit_core::PreparedStructuredValueValidator::storage_reservation(ty, maximum)
                .map_err(TraversalStorageRefusal::CoreStorage)?;
        let extra = add(
            ty.owned_heap_bytes(),
            ty.canonical_byte_length()
                .map_err(TraversalStorageRefusal::Structure)?,
        )?;
        Ok(TraversalStorageReceipt {
            preparation_requested_bytes_bound: add(inner.preparation_requested_bytes_bound, extra)?,
            retained_heap_bytes_bound: add(inner.retained_heap_bytes_bound, extra)?,
            retained_heap_bytes: 0,
        })
    }
    pub fn new_with_storage_limits(
        ty: &StructuredInfoType,
        maximum: usize,
        preparation: usize,
        retained: usize,
    ) -> Result<(Self, TraversalStorageReceipt), TraversalStorageRefusal> {
        let mut reservation = Self::storage_reservation(ty, maximum)?;
        if reservation.preparation_requested_bytes_bound > preparation
            || reservation.retained_heap_bytes_bound > retained
        {
            return Err(TraversalStorageRefusal::Capacity);
        }
        let value = Self::new(ty, maximum).map_err(TraversalStorageRefusal::Structure)?;
        reservation.retained_heap_bytes = value.owned_heap_bytes();
        if reservation.retained_heap_bytes > reservation.retained_heap_bytes_bound {
            return Err(TraversalStorageRefusal::Capacity);
        }
        Ok((value, reservation))
    }
}
impl PreparedStructuredContractValidator {
    pub fn owned_heap_bytes(&self) -> usize {
        self.contracts.iter().fold(
            self.structure.owned_heap_bytes().saturating_add(
                self.contracts
                    .capacity()
                    .saturating_mul(size_of::<(String, CheckedValueContract)>()),
            ),
            |total, (path, contract)| {
                total
                    .saturating_add(path.capacity())
                    .saturating_add(contract.owned_heap_bytes())
            },
        )
    }
    pub fn storage_reservation(
        ty: &StructuredInfoType,
        maximum: usize,
        contracts: &[(String, CheckedValueContract)],
    ) -> Result<TraversalStorageReceipt, TraversalStorageRefusal> {
        let mut reservation = PreparedStructuredValueValidator::storage_reservation(ty, maximum)?;
        let mut prep = array::<(String, CheckedValueContract)>(contracts.len())?;
        let mut retained = prep;
        for (path, contract) in contracts {
            let clone = contract
                .clone_storage_reservation()
                .map_err(TraversalStorageRefusal::ContractStorage)?;
            prep = add(
                add(prep, path.len())?,
                clone.preparation_requested_bytes_bound,
            )?;
            retained = add(add(retained, path.len())?, clone.retained_heap_bytes_bound)?;
        }
        reservation.preparation_requested_bytes_bound =
            add(reservation.preparation_requested_bytes_bound, prep)?;
        reservation.retained_heap_bytes_bound =
            add(reservation.retained_heap_bytes_bound, retained)?;
        Ok(reservation)
    }
    pub fn new_with_storage_limits(
        ty: &StructuredInfoType,
        maximum: usize,
        contracts: &[(String, CheckedValueContract)],
        preparation: usize,
        retained: usize,
    ) -> Result<(Self, TraversalStorageReceipt), TraversalStorageRefusal> {
        let mut reservation = Self::storage_reservation(ty, maximum, contracts)?;
        if reservation.preparation_requested_bytes_bound > preparation
            || reservation.retained_heap_bytes_bound > retained
        {
            return Err(TraversalStorageRefusal::Capacity);
        }
        let value = Self::new(ty, maximum, contracts).map_err(TraversalStorageRefusal::Contract)?;
        reservation.retained_heap_bytes = value.owned_heap_bytes();
        if reservation.retained_heap_bytes > reservation.retained_heap_bytes_bound {
            return Err(TraversalStorageRefusal::Capacity);
        }
        Ok((value, reservation))
    }
}
