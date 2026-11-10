//! Fallible pre-Play funding of the existing canonical slot representation.
use super::*;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueStorePreparationError {
    Storage(StorageError),
    Capacity { required: usize, available: usize },
    Allocation,
}
impl HostedValueStore {
    /// Prepare the existing canonical store under a requested-backing ceiling.
    /// Slots and payloads are all allocated here; publication cannot grow them.
    /// Inline owner storage and allocator overhead are separate Host charges.
    pub fn try_new_with_slot_capacities(
        capacities: &[u32],
        byte_capacity: u32,
        maximum_backing_bytes: usize,
    ) -> Result<Self, ValueStorePreparationError> {
        use ValueStorePreparationError as E;
        let required =
            Self::canonical_storage_bytes(capacities.iter().copied()).map_err(E::Storage)?;
        if required > maximum_backing_bytes {
            return Err(E::Capacity {
                required,
                available: maximum_backing_bytes,
            });
        }
        let payload = capacities
            .iter()
            .try_fold(0u32, |sum, c| sum.checked_add(*c))
            .ok_or(E::Storage(StorageError::InvalidBudget))?;
        if byte_capacity == 0 || byte_capacity > payload {
            return Err(E::Storage(StorageError::InvalidBudget));
        }
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(capacities.len())
            .map_err(|_| E::Allocation)?;
        let mut backing = slots
            .capacity()
            .checked_mul(core::mem::size_of::<HostedValueSlot>())
            .ok_or(E::Storage(StorageError::InvalidBudget))?;
        if backing > maximum_backing_bytes {
            return Err(E::Capacity {
                required: backing,
                available: maximum_backing_bytes,
            });
        }
        for capacity in capacities {
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(
                    usize::try_from(*capacity)
                        .map_err(|_| E::Storage(StorageError::InvalidBudget))?,
                )
                .map_err(|_| E::Allocation)?;
            backing = backing
                .checked_add(bytes.capacity())
                .ok_or(E::Storage(StorageError::InvalidBudget))?;
            if backing > maximum_backing_bytes {
                return Err(E::Capacity {
                    required: backing,
                    available: maximum_backing_bytes,
                });
            }
            slots.push(HostedValueSlot {
                generation: 0,
                references: 0,
                bytes,
            });
        }
        let maximum = usize::try_from(
            *capacities
                .iter()
                .max()
                .expect("validated nonempty capacities"),
        )
        .map_err(|_| E::Storage(StorageError::InvalidBudget))?;
        Ok(Self::from_prepared_slots(slots, maximum, byte_capacity))
    }
}
