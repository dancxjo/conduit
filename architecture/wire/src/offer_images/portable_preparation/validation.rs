//! Additional validation backing for the closed portable scalar profile.
//! Pinned alloc::btree recipe (Rust 1.98.1): 11 keys and 12 child edges.
//! This does not include typed records, parser/inflater, stack or transactions.
use super::{Refusal, ValidationShape};
use core::{
    alloc::Layout,
    mem::{align_of, size_of},
    ptr::NonNull,
};

fn add(a: usize, b: usize) -> Result<usize, Refusal> {
    a.checked_add(b).ok_or(Refusal::Overflow)
}
fn node<K>() -> Result<usize, Refusal> {
    let alignment = align_of::<K>()
        .max(align_of::<NonNull<()>>())
        .max(align_of::<u16>());
    // Leaf has four nonzero fields: parent pointer, parent index, length,
    // key array. Bound all field-order and final padding, rather than assuming
    // private repr(Rust) offsets. Values are SetValZST (size0/alignment1).
    let keys = size_of::<K>().checked_mul(11).ok_or(Refusal::Overflow)?;
    let padding = (alignment - 1).checked_mul(4).ok_or(Refusal::Overflow)?;
    let bytes = add(
        add(
            add(size_of::<Option<NonNull<()>>>(), 2 * size_of::<u16>())?,
            keys,
        )?,
        padding,
    )?;
    let leaf = Layout::from_size_align(bytes, alignment)
        .map_err(|_| Refusal::Overflow)?
        .pad_to_align();
    let edges = Layout::array::<NonNull<()>>(12).map_err(|_| Refusal::Overflow)?;
    Ok(leaf
        .extend(edges)
        .map_err(|_| Refusal::Overflow)?
        .0
        .pad_to_align()
        .size())
}
fn tree<K>(attempts: usize) -> Result<usize, Refusal> {
    if attempts == 0 {
        return Ok(0);
    }
    // Populated nodes own keys; the extra node covers insertion/root-growth
    // intermediates. Count attempted inserts, including duplicate location keys
    // whose String is cloned before duplicate detection.
    add(attempts, 1)?
        .checked_mul(node::<K>()?)
        .ok_or(Refusal::Overflow)
}
pub(super) fn backing(shape: ValidationShape) -> Result<usize, Refusal> {
    let outer = add(
        tree::<&str>(shape.port_symbols)?,
        tree::<&str>(shape.configuration_keys)?,
    )?;
    let locations = add(
        tree::<conduit_core::FrontValueLocation>(shape.maximum_value_locations)?,
        shape.maximum_value_location_string_bytes,
    )?;
    let laws = add(tree::<&str>(shape.terminal_inputs)?, locations)?;
    add(outer, shape.configuration_kind_bytes.max(laws))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_and_string_only_validation_phases_are_distinct() {
        assert_eq!(backing(ValidationShape::default()).unwrap(), 0);
        assert_eq!(
            backing(ValidationShape {
                configuration_kind_bytes: 11,
                ..ValidationShape::default()
            })
            .unwrap(),
            11
        );
    }
    #[test]
    fn key_specific_nodes_and_location_strings_are_separate_terms() {
        assert!(node::<&str>().unwrap() > 11 * size_of::<&str>());
        assert!(node::<conduit_core::FrontValueLocation>().unwrap() > node::<&str>().unwrap());
        let shape = ValidationShape {
            port_symbols: 12,
            configuration_keys: 2,
            terminal_inputs: 3,
            maximum_value_locations: 13,
            maximum_value_location_string_bytes: 4096,
            configuration_kind_bytes: 11,
        };
        let expected = tree::<&str>(12).unwrap()
            + tree::<&str>(2).unwrap()
            + tree::<&str>(3).unwrap()
            + tree::<conduit_core::FrontValueLocation>(13).unwrap()
            + 4096;
        assert_eq!(backing(shape).unwrap(), expected);
    }
    #[test]
    fn oversized_validation_shapes_refuse_instead_of_wrapping() {
        assert!(tree::<&str>(usize::MAX).is_err());
        assert!(backing(ValidationShape {
            maximum_value_location_string_bytes: usize::MAX,
            configuration_kind_bytes: 0,
            maximum_value_locations: 1,
            ..ValidationShape::default()
        })
        .is_err());
    }
}
