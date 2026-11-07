//! Canonical Source-derived value storage shared by MCU platforms.
extern crate alloc;
pub type PreparedValues<const W: usize> = (
    conduit_kernel::HostedValueStore,
    [(u16, conduit_kernel::ValueRef); W],
);

/// Prepare only independently compiled canonical payload extents. Clock request
/// words retain their own actual values; native result extents are not inferred
/// from the (potentially much smaller) outgoing typed Cord values.
pub fn prepare<const W: usize>(
    cords: &[u32],
    steps: &[u32],
    results: &[u32],
    clocks: &[(u16, u64, u64); W],
    maximum_backing: usize,
) -> Result<PreparedValues<W>, &'static str> {
    use conduit_kernel::ValueStorage;
    let count = cords
        .len()
        .checked_add(steps.len())
        .and_then(|n| n.checked_add(results.len()))
        .and_then(|n| n.checked_add(W))
        .ok_or("value-count")?;
    let mut capacities = alloc::vec::Vec::new();
    capacities
        .try_reserve_exact(count)
        .map_err(|_| "value-capacities")?;
    capacities.extend_from_slice(cords);
    capacities.extend_from_slice(steps);
    capacities.extend_from_slice(results);
    capacities.extend(core::iter::repeat_n(8, W));
    let bytes = capacities
        .iter()
        .try_fold(0u32, |n, c| n.checked_add(*c))
        .ok_or("value-bytes")?;
    let mut store = conduit_kernel::HostedValueStore::try_new_with_slot_capacities(
        &capacities,
        bytes,
        maximum_backing,
    )
    .map_err(|_| "value-storage")?;
    // Each actual Source clock has an independent retained canonical parameter.
    // The general store chooses a fitting slot; no ValueRef is manufactured.
    let mut parameters = [(
        0,
        conduit_kernel::ValueRef {
            slot: 0,
            generation: 0,
            byte_len: 0,
        },
    ); W];
    for (slot, (node, period, count)) in clocks.iter().enumerate() {
        if *period == 0 || *count == 0 {
            return Err("clock-geometry");
        }
        parameters[slot] = (
            *node,
            store
                .store(&period.to_le_bytes())
                .map_err(|_| "clock-parameter")?,
        );
    }
    Ok((store, parameters))
}
