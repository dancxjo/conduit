//! Exact preparation-time transfer into caller-owned fixed target storage.
//! No allocation, resource discovery, schema reconstruction or model copy.
//! Call only before Play. Caller owns storage placement and the sealed Plan.
use super::{FixedValueStore, StorageError, ValueRef, ValueStorage};

#[derive(Debug, Eq, PartialEq)]
pub enum FixedIngressRefusal {
    DestinationNotEmpty,
    IncompleteIngress,
    Order,
    ReferenceIdentity,
    Storage(StorageError),
}

/// `live` must enumerate every live reference in increasing slot order, once.
/// Exact identity is checked after each insertion; a failed preparation clears
/// the destination and leaves the original admitted storage untouched.
pub fn transfer<const SLOTS: usize, const BYTES: usize>(
    source: &impl ValueStorage,
    destination: &mut FixedValueStore<SLOTS, BYTES>,
    live: &[ValueRef],
) -> Result<(), FixedIngressRefusal> {
    if destination.used_items() != 0 || destination.used_bytes() != 0 {
        return Err(FixedIngressRefusal::DestinationNotEmpty);
    }
    let result = (|| {
        if usize::from(source.used_items()) != live.len() {
            return Err(FixedIngressRefusal::IncompleteIngress);
        }
        for (index, reference) in live.iter().copied().enumerate() {
            if usize::from(reference.slot) != index {
                return Err(FixedIngressRefusal::Order);
            }
            let bytes = source
                .get(reference)
                .map_err(FixedIngressRefusal::Storage)?;
            let copied = destination
                .store(bytes)
                .map_err(FixedIngressRefusal::Storage)?;
            if copied != reference {
                return Err(FixedIngressRefusal::ReferenceIdentity);
            }
            let count = source
                .reference_count(reference)
                .map_err(FixedIngressRefusal::Storage)?;
            for _ in 1..count {
                destination
                    .retain(copied)
                    .map_err(FixedIngressRefusal::Storage)?;
            }
        }
        if source.used_bytes() != destination.used_bytes() {
            return Err(FixedIngressRefusal::IncompleteIngress);
        }
        Ok(())
    })();
    if result.is_err() {
        destination.clear();
    }
    result
}

#[cfg(test)]
mod tests;
