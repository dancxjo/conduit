//! Physical stock from exact prepared-owner lifetimes and compiled topology.
//! Logical references, byte quotas, Cord capacities and canonical storage stay
//! unchanged. This constructor neither admits resources nor authorizes a Play.
extern crate alloc;
use super::{prepare, ValueSlotProfile as Profile};
use crate::execution::{PreparedBacks, RetainedInputs};
use alloc::vec::Vec;
use conduit_core::BorrowedStructuredValueValidator as Validator;
use conduit_plan_lowering::shared_schema_value_store::SharedSchemaValueStore;

pub type PreparedSharedSchemaValues<const W: usize> = (
    SharedSchemaValueStore<'static, conduit_kernel::HostedValueStore>,
    [(u16, conduit_kernel::ValueRef); W],
    SharedStockReport,
);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SharedStockReport {
    pub logical_schema_slots: usize,
    pub physical_schema_slots: usize,
    pub reduced_classes: usize,
    /// Original physical stock is retained for every class if any owner lacks
    /// an audited bound. This does not newly prove the original Plan adequate.
    pub unknown_owner_fallback: bool,
}
struct Class {
    validator: Validator<'static>,
    logical: u16,
    physical: u16,
}
fn schema(profile: &Profile) -> Option<(&'static [u8], u32)> {
    match *profile {
        Profile::Schema {
            canonical,
            maximum_bytes,
        } => Some((canonical, maximum_bytes)),
        Profile::Canonical(_) => None,
    }
}
/// Single-Step scheduling permits sharing transient output stock between nodes,
/// but not between simultaneous outputs of the same node. Every original queue
/// slot remains funded, as do held states and outstanding native input pins.
/// The caller supplies topology emitted alongside the original profiles from
/// its independently checked Source; actual Back reports come from the owners
/// later bound by the same factory. Neither discovery nor Kind names suffice.
#[allow(clippy::too_many_arguments)]
pub fn prepare_shared_schema<const W: usize, const N: usize, const P: usize>(
    cords: &[Profile],
    sinks: &[Option<(u16, u16)>],
    steps: &[Profile],
    outputs: &[(u16, u16)],
    results: &[u32],
    clocks: &[(u16, u64, u64); W],
    maximum_backing: usize,
    backs: &impl PreparedBacks<N, P>,
) -> Result<PreparedSharedSchemaValues<W>, &'static str> {
    if cords.len() != sinks.len() || steps.len() != outputs.len() {
        return Err("value-topology-length");
    }
    for (node, port) in sinks.iter().flatten().chain(outputs) {
        if usize::from(*node) >= N || usize::from(*port) >= P {
            return Err("value-topology-port");
        }
    }
    for (index, output) in outputs.iter().enumerate() {
        if outputs[..index].contains(output) {
            return Err("value-topology-output");
        }
    }
    let count = cords.len().checked_add(steps.len()).ok_or("value-count")?;
    let mut canonical = Vec::new();
    let mut logical = Vec::new();
    let mut classes: Vec<Class> = Vec::new();
    canonical
        .try_reserve_exact(count)
        .map_err(|_| "value-capacities")?;
    logical
        .try_reserve_exact(count)
        .map_err(|_| "value-definitions")?;
    classes
        .try_reserve_exact(count)
        .map_err(|_| "value-classes")?;
    let mut logical_bytes = 0u32;
    for profile in cords.iter().chain(steps) {
        if let Some((bytes, maximum)) = schema(profile) {
            let validator = Validator::new(
                bytes,
                usize::try_from(maximum).map_err(|_| "value-schema-width")?,
            )
            .map_err(|_| "value-schema")?;
            logical_bytes = logical_bytes.checked_add(maximum).ok_or("value-bytes")?;
            logical.push((validator, 1));
            if let Some(class) = classes
                .iter_mut()
                .find(|c| c.validator.type_bytes() == bytes)
            {
                class.logical = class.logical.checked_add(1).ok_or("value-count")?;
                if class.validator.maximum_bytes() < validator.maximum_bytes() {
                    class.validator = validator;
                }
            } else {
                classes.push(Class {
                    validator,
                    logical: 1,
                    physical: 0,
                });
            }
        } else if let Profile::Canonical(bytes) = *profile {
            canonical.push(bytes);
        }
    }
    if classes.is_empty() {
        return Err("value-schema-profile-empty");
    }
    let unknown = (0..N).any(|node| {
        matches!(backs.retained_inputs(node), RetainedInputs::Unknown)
            || backs.pending_input_pins(node).is_none()
    });
    let mut reduced = 0;
    for class in &mut classes {
        let bytes = class.validator.type_bytes();
        let is_class = |profile: &Profile| schema(profile).is_some_and(|(s, _)| s == bytes);
        let queue_stock = cords.iter().filter(|p| is_class(p)).count();
        let mut transient = 0;
        let mut retained = 0usize;
        for node in 0..N {
            let staged = steps
                .iter()
                .zip(outputs)
                .filter(|(p, (n, _))| usize::from(*n) == node && is_class(p))
                .count();
            transient = transient.max(staged);
            let eligible = |ports: &[bool; P]| {
                cords.iter().zip(sinks).any(|(p, sink)| {
                    sink.is_some_and(|(n, port)| usize::from(n) == node && ports[usize::from(port)])
                        && is_class(p)
                })
            };
            match backs.retained_inputs(node) {
                RetainedInputs::Bounded {
                    maximum_values,
                    possible_ports,
                } => {
                    if maximum_values != 0 && !possible_ports.iter().any(|p| *p) {
                        return Err("value-retention-ports");
                    }
                    if eligible(&possible_ports) {
                        retained = retained
                            .checked_add(usize::from(maximum_values))
                            .ok_or("value-count")?;
                    }
                }
                RetainedInputs::Unknown => {}
            }
            // Every possible native pin is funded separately, rather than
            // interpreting a Back's zero held references as zero kernel custody.
            if let Some(pins) = backs.pending_input_pins(node) {
                for port in 0..P {
                    if pins[port] {
                        let mut one = [false; P];
                        one[port] = true;
                        if eligible(&one) {
                            retained = retained.checked_add(1).ok_or("value-count")?;
                        }
                    }
                }
            }
        }
        let required = queue_stock
            .checked_add(transient)
            .and_then(|n| n.checked_add(retained))
            .ok_or("value-count")?;
        // Never clamp an unmet bound and call it a lifetime proof. Keep the
        // original stock when no reduction is established, including all its
        // original heterogeneous extents (below), without a new adequacy claim.
        class.physical = if !unknown && required > 0 && required < usize::from(class.logical) {
            reduced += 1;
            required as u16
        } else {
            class.logical
        };
    }
    let mut physical = Vec::new();
    physical
        .try_reserve_exact(logical.len())
        .map_err(|_| "value-stock")?;
    for class in &classes {
        if class.physical < class.logical {
            physical.push((class.validator, class.physical));
        } else {
            physical.extend(
                logical
                    .iter()
                    .copied()
                    .filter(|(v, _)| v.type_bytes() == class.validator.type_bytes()),
            );
        }
    }
    let report = SharedStockReport {
        logical_schema_slots: logical.len(),
        physical_schema_slots: physical.iter().map(|(_, n)| usize::from(*n)).sum(),
        reduced_classes: reduced,
        unknown_owner_fallback: unknown,
    };
    let (canonical, parameters) = prepare(&canonical, &[], results, clocks, maximum_backing)?;
    let remaining = maximum_backing
        .checked_sub(canonical.reserved_storage_bytes())
        .ok_or("value-backing")?;
    let store =
        SharedSchemaValueStore::try_new(canonical, &logical, &physical, logical_bytes, remaining)
            .map_err(|_| "value-shared-storage")?;
    Ok((store, parameters, report))
}

#[cfg(test)]
#[path = "native_value_stock_tests.rs"]
mod tests;
