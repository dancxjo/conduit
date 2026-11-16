//! Exact equivalence and bounded work for invocation-local fingerprint reuse.
extern crate std;
use super::*;
#[test]
fn prefix_reuse_matches_original_fingerprint_even_when_primary_keys_change() {
    let keys = [
        ("", ""),
        ("", "suffix"),
        ("body/α", "name"),
        ("body/α", "role"),
        ("body/β", "role"),
        ("body/α", "name"),
        ("key/124", ""),
        ("key/336", ""),
        ("body/α\0", "name"),
    ];
    let mut cache = FingerprintPrefix::new();
    for key in keys {
        assert_eq!(cache.fingerprint(key), fingerprint(key));
    }
}
#[test]
fn repeated_primary_keys_reduce_hashed_bytes_without_retaining_an_index_cache() {
    let primary = alloc::format!("plot/{}/gear/{}/output", "a".repeat(64), "b".repeat(64));
    let suffixes: alloc::vec::Vec<_> = (0..16).map(|i| alloc::format!("property-{i:02}")).collect();
    let mut cache = FingerprintPrefix::new();
    let mut original_bytes = 0;
    for suffix in &suffixes {
        let key = (primary.as_str(), suffix.as_str());
        assert_eq!(cache.fingerprint(key), fingerprint(key));
        original_bytes += key.0.len() + key.1.len() + 2;
    }
    assert_eq!(
        cache.hashed_bytes,
        primary.len() + 1 + suffixes.iter().map(|s| s.len() + 1).sum::<usize>()
    );
    assert!(cache.hashed_bytes * 4 < original_bytes);
    // A separate invocation always computes its own original first prefix.
    let mut next = FingerprintPrefix::new();
    assert_eq!(
        next.fingerprint((&primary, &suffixes[0])),
        fingerprint((&primary, &suffixes[0]))
    );
    assert_eq!(next.hashed_bytes, primary.len() + suffixes[0].len() + 2);
    std::eprintln!("Fingerprint work fixture: original hashed bytes={original_bytes}, invocation-local prefix reuse={}; this is not an RTC fit claim", cache.hashed_bytes);
}
#[test]
fn bulk_membership_preserves_collisions_repeated_keys_and_complete_value_equality() {
    let values = [
        ("key/124", "", 3),
        ("key/336", "", 9),
        ("key/124", "", 7),
        ("same", "left", 1),
        ("same", "right", 2),
    ];
    assert_eq!(
        fingerprint((values[0].0, values[0].1)),
        fingerprint((values[1].0, values[1].1))
    );
    let index = Index::<_, 8>::new(&values, |v| (v.0, v.1)).unwrap();
    for first in ["key/124", "key/336", "same", "absent"] {
        for second in ["", "left", "right"] {
            for value in 0..12 {
                let query = [(first, second, value), ("same", "right", 2)];
                assert_eq!(
                    index.contains_all(&query),
                    query.iter().all(|q| values.contains(q))
                );
            }
        }
    }
    assert!(index.contains_all(&[]));
    assert!(index.contains_all(&values));
    assert!(index.contains_all(&[values[0], values[0]]));
    assert!(
        !index.contains_all(&[("key/336", "", 3)]),
        "fingerprint collision cannot substitute a value"
    );
}
