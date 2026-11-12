//! Challenge the pinned Rust 1.98.1 collection terms used in the preparation
//! derivation. These terms alone are not a complete constructor/admission bound.
use conduit_core::PlacementId;
use conduit_kernel::NodeId;
use std::collections::{BTreeMap, BTreeSet};
use std::mem::{align_of, size_of};

// Pinned alloc::raw_vec growth, including all capacity epochs rather than only
// the final capacity. Summing requests also covers realloc-copy overlap.
fn vector_requests<T>(count: usize) -> Option<usize> {
    let size = size_of::<T>();
    if size == 0 {
        return Some(0);
    }
    let minimum = if size == 1 {
        8
    } else if size <= 1024 {
        4
    } else {
        1
    };
    let mut capacity = 0usize;
    let mut bytes = 0usize;
    for len in 0..count {
        if len == capacity {
            capacity = capacity
                .checked_mul(2)?
                .max(len.checked_add(1)?)
                .max(minimum);
            bytes = bytes.checked_add(capacity.checked_mul(size)?)?;
        }
    }
    Some(bytes)
}
fn rounded(value: usize, alignment: usize) -> Option<usize> {
    value
        .checked_add(alignment - 1)
        .map(|n| n / alignment * alignment)
}
// Pinned B=6 leaf fields, with conservative padding for each field. Nested key
// owners are separate. This challenges the pinned implementation, not Rust's
// general repr(Rust) contract or an arbitrary future standard library.
fn leaf_upper<K, V>() -> Option<usize> {
    let alignment = align_of::<usize>()
        .max(align_of::<K>())
        .max(align_of::<V>());
    rounded(size_of::<usize>(), alignment)?
        .checked_add(rounded(size_of::<u16>(), alignment)?.checked_mul(2)?)?
        .checked_add(rounded(11usize.checked_mul(size_of::<K>())?, alignment)?)?
        .checked_add(rounded(11usize.checked_mul(size_of::<V>())?, alignment)?)
}
#[test]
fn pinned_small_tree_leaf_terms_cover_actual_requested_layouts() {
    for count in 0..=11 {
        let keys: Vec<_> = (0..count)
            .map(|n| PlacementId::from(format!("fixture/{n}")))
            .collect();
        let probe = crate::allocation_probe::begin();
        let mut map = BTreeMap::new();
        for (n, key) in keys.into_iter().enumerate() {
            map.insert(key, NodeId(n as u16));
        }
        std::hint::black_box(&map);
        drop(map);
        let observed = probe.finish_statistics();
        let bound = if count == 0 {
            0
        } else {
            leaf_upper::<PlacementId, NodeId>().unwrap()
        };
        assert!(
            observed.requested_bytes <= bound,
            "{count}: {observed:?} > {bound}"
        );
        let keys: Vec<_> = (0..count).map(|n| format!("fixture/{n}")).collect();
        let probe = crate::allocation_probe::begin();
        let mut set = BTreeSet::new();
        for key in keys {
            set.insert(key);
        }
        std::hint::black_box(&set);
        drop(set);
        let observed = probe.finish_statistics();
        let bound = if count == 0 {
            0
        } else {
            leaf_upper::<String, ()>().unwrap()
        };
        assert!(
            observed.requested_bytes <= bound,
            "{count}: {observed:?} > {bound}"
        );
    }
}
#[test]
fn pinned_push_and_result_collection_epochs_cover_requested_bytes() {
    for count in 0..=20 {
        let probe = crate::allocation_probe::begin();
        let mut pushed = Vec::new();
        for n in 0..count {
            pushed.push(NodeId(n as u16));
        }
        std::hint::black_box(&pushed);
        drop(pushed);
        let observed = probe.finish_statistics();
        let bound = vector_requests::<NodeId>(count).unwrap();
        assert!(
            observed.requested_bytes <= bound,
            "push {count}: {observed:?} > {bound}"
        );
        let probe = crate::allocation_probe::begin();
        let collected = (0..count)
            .map(|n| Ok::<_, ()>(NodeId(n as u16)))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        std::hint::black_box(&collected);
        drop(collected);
        let observed = probe.finish_statistics();
        assert!(
            observed.requested_bytes <= bound,
            "collect {count}: {observed:?} > {bound}"
        );
    }
}
#[test]
fn pinned_small_stable_sort_uses_no_heap_workspace() {
    for count in 0..=20 {
        let mut values: Vec<_> = (0..count)
            .rev()
            .map(|n| PlacementId::from(format!("fixture/{n:02}")))
            .collect();
        let probe = crate::allocation_probe::begin();
        values.sort();
        let observed = probe.finish_statistics();
        assert_eq!(observed.allocations, 0, "{count}: {observed:?}");
    }
}
