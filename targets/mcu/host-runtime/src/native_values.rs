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

/// One independently compiled physical slot selection. Canonical quotas remain
/// full value extents; linked schema bytes are never counted as node backing.
#[cfg(feature = "schema-value-storage")]
#[derive(Clone, Copy)]
pub enum ValueSlotProfile {
    Canonical(u32),
    Schema {
        canonical: &'static [u8],
        maximum_bytes: u32,
    },
}
#[cfg(feature = "schema-value-storage")]
pub type PreparedSchemaValues<const W: usize> = (
    conduit_plan_lowering::schema_value_store::SchemaValueStore<
        'static,
        conduit_kernel::HostedValueStore,
    >,
    [(u16, conduit_kernel::ValueRef); W],
);

/// Construct the exact selected physical pool before Play. This grants no
/// resource or semantic admission. The original retained-backing ceiling funds
/// both owners, including actual buffer capacities and slot metadata. Root
/// independently funds temporary preparation scratch and linked schema custody.
#[cfg(feature = "schema-value-storage")]
pub fn prepare_schema<const W: usize>(
    profiles: &[ValueSlotProfile],
    steps: &[ValueSlotProfile],
    results: &[u32],
    clocks: &[(u16, u64, u64); W],
    maximum_backing: usize,
) -> Result<PreparedSchemaValues<W>, &'static str> {
    use conduit_core::BorrowedStructuredValueValidator;
    use conduit_plan_lowering::schema_value_store::SchemaValueStore;
    let mut canonical = alloc::vec::Vec::new();
    let mut definitions = alloc::vec::Vec::new();
    let count = profiles
        .len()
        .checked_add(steps.len())
        .ok_or("value-count")?;
    canonical
        .try_reserve_exact(count)
        .map_err(|_| "value-capacities")?;
    definitions
        .try_reserve_exact(count)
        .map_err(|_| "value-definitions")?;
    let mut logical = 0u32;
    for profile in profiles.iter().chain(steps) {
        match *profile {
            ValueSlotProfile::Canonical(bytes) => canonical.push(bytes),
            ValueSlotProfile::Schema {
                canonical,
                maximum_bytes,
            } => {
                let validator = BorrowedStructuredValueValidator::new(
                    canonical,
                    usize::try_from(maximum_bytes).map_err(|_| "value-schema-width")?,
                )
                .map_err(|_| "value-schema")?;
                logical = logical.checked_add(maximum_bytes).ok_or("value-bytes")?;
                definitions.push((validator, 1));
            }
        }
    }
    if definitions.is_empty() {
        return Err("value-schema-profile-empty");
    }
    let (canonical, parameters) = prepare(&canonical, &[], results, clocks, maximum_backing)?;
    let remaining = maximum_backing
        .checked_sub(canonical.reserved_storage_bytes())
        .ok_or("value-backing")?;
    let store = SchemaValueStore::try_new(canonical, &definitions, logical, remaining)
        .map_err(|_| "value-schema-storage")?;
    Ok((store, parameters))
}

#[cfg(all(test, feature = "schema-value-storage"))]
#[path = "native_value_profile_tests.rs"]
mod profile_tests;
