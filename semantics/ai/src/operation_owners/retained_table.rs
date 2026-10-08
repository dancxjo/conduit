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
