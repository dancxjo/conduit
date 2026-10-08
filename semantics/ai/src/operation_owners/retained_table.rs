//! Moves transient preparation selections into bounded sorted runtime storage.
//! This does not admit the transient map/catalog/checker preparation peak.
use alloc::{collections::BTreeMap, string::String};
use conduit_core::bounded_owner_table::BoundedOwnerTable;
pub(super) fn retain<K: Ord, V>(map: BTreeMap<K, V>) -> Result<BoundedOwnerTable<K, V>, String> {
    let r = BoundedOwnerTable::<K, V>::storage_reservation(map.len())
        .map_err(|_| String::from("retained owner table capacity"))?;
    let (mut table, _) = BoundedOwnerTable::with_storage_limits(
        map.len(),
        r.preparation_requested_bytes_bound,
        r.retained_array_bytes_bound,
    )
    .map_err(|_| String::from("retained owner table capacity"))?;
    for (key, value) in map {
        table
            .try_insert(key, value)
            .map_err(|_| String::from("retained owner table duplicate or capacity"))?;
    }
    Ok(table)
}

pub(super) fn retain_nested<K: Ord, I: Ord, V>(
    map: BTreeMap<K, BTreeMap<I, V>>,
) -> Result<BoundedOwnerTable<K, BoundedOwnerTable<I, V>>, String> {
    let r = BoundedOwnerTable::<K, BoundedOwnerTable<I, V>>::storage_reservation(map.len())
        .map_err(|_| String::from("retained nested owner table capacity"))?;
    let (mut table, _) = BoundedOwnerTable::with_storage_limits(
        map.len(),
        r.preparation_requested_bytes_bound,
        r.retained_array_bytes_bound,
    )
    .map_err(|_| String::from("retained nested owner table capacity"))?;
    for (key, value) in map {
        table
            .try_insert(key, retain(value)?)
            .map_err(|_| String::from("retained nested owner table duplicate or capacity"))?;
    }
    Ok(table)
}

pub(super) fn tensor_factory_local_payload(
    identity: &conduit_core::ImplementationId,
    offers: &BoundedOwnerTable<conduit_core::CapabilityId, conduit_core::CapabilityOffer>,
    resources: &BoundedOwnerTable<
        conduit_core::PlacementId,
        BoundedOwnerTable<
            String,
            alloc::sync::Arc<crate::fixed_tensor_resource::AdmittedFixedTensorResource>,
        >,
    >,
) -> Option<usize> {
    let mut total = identity
        .owned_heap_bytes()
        .checked_add(offers.array_capacity_bytes().ok()?)?
        .checked_add(resources.array_capacity_bytes().ok()?)?;
    for (key, offer) in offers.iter() {
        total = total
            .checked_add(key.owned_heap_bytes())?
            .checked_add(conduit_core::capability_offer_owned_heap_bytes(offer).ok()?)?;
    }
    for (placement, bindings) in resources.iter() {
        total = total
            .checked_add(placement.owned_heap_bytes())?
            .checked_add(bindings.array_capacity_bytes().ok()?)?;
        for (key, _) in bindings.iter() {
            total = total.checked_add(key.capacity())?;
        }
    }
    Some(total)
}
