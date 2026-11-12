//! Physical geometry shared with FixedHostCallBindings' node/operation indexing.
//! Derived from checked lowering, then funded through the existing kernel size.
pub(super) fn slot_count(
    nodes: usize,
    calls: impl Iterator<Item = u16>,
) -> Result<usize, &'static str> {
    let mut stride = 1u16;
    for call in calls {
        stride = stride.max(call.checked_add(1).ok_or("Host Call stride overflow")?);
    }
    nodes
        .checked_mul(usize::from(stride))
        .ok_or("Host Call table overflow")
}

/// The scheduler owns at most one pending Host Call per node, including a
/// completed request until its result is consumed. Distinct operations on one
/// node share this slot; their sparse binding addresses remain unchanged.
/// Boxed scheduler preparation requires one slot even for a call-free fragment.
pub(super) fn pending_count(
    nodes: usize,
    calls: impl Iterator<Item = (u16, u16)>,
) -> Result<usize, &'static str> {
    let mut callable = std::collections::BTreeSet::new();
    for (node, maximum_in_flight) in calls {
        if usize::from(node) >= nodes || maximum_in_flight != 1 {
            return Err("unsupported pending Host Call geometry");
        }
        callable.insert(node);
    }
    Ok(callable.len().max(1))
}

#[cfg(test)]
mod tests {
    use super::{pending_count, slot_count};

    #[test]
    fn no_calls_and_single_operation_keep_stride_one() {
        assert_eq!(slot_count(18, [].into_iter()), Ok(18));
        assert_eq!(slot_count(18, [0, 0].into_iter()), Ok(18));
    }

    #[test]
    fn two_operations_fund_the_last_node_instead_of_packing_records() {
        let slots = slot_count(18, [0, 0, 0, 1].into_iter()).unwrap();
        assert_eq!(slots, 36);
        assert_eq!(17 * 2 + 1, slots - 1);
    }

    #[test]
    fn sparse_ids_and_overflow_follow_receiver_indexing() {
        assert_eq!(slot_count(3, [4].into_iter()), Ok(15));
        assert!(slot_count(3, [u16::MAX].into_iter()).is_err());
        assert!(slot_count(usize::MAX, [1].into_iter()).is_err());
    }

    #[test]
    fn pending_slots_count_callable_nodes_independently_of_binding_stride() {
        let calls = [(0, 1), (9, 0), (4, 0), (0, 0)];
        assert_eq!(slot_count(18, calls.iter().map(|(_, call)| *call)), Ok(36));
        assert_eq!(
            pending_count(18, calls.iter().map(|(node, _)| (*node, 1))),
            Ok(3)
        );
        assert_eq!(slot_count(3, [4].into_iter()), Ok(15));
        assert_eq!(pending_count(3, [(2, 1)].into_iter()), Ok(1));
    }

    #[test]
    fn pending_slots_keep_empty_constructor_valid_and_refuse_unproven_geometry() {
        assert_eq!(pending_count(18, [].into_iter()), Ok(1));
        assert!(pending_count(3, [(3, 1)].into_iter()).is_err());
        assert!(pending_count(3, [(0, 0)].into_iter()).is_err());
        assert!(pending_count(3, [(0, 2)].into_iter()).is_err());
    }
}
