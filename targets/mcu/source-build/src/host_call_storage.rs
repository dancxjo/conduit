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

#[cfg(test)]
mod tests {
    use super::slot_count;

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
}
