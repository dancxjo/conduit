//! A temporary search accelerator over the complete immutable records.
//! Keys narrow the search; only full equality establishes membership.
use alloc::vec::Vec;

pub(super) struct Lookup<'a, T> {
    values: &'a [T],
    key: fn(&T) -> &str,
    order: Vec<usize>,
}

impl<'a, T: PartialEq> Lookup<'a, T> {
    pub(super) fn new(values: &'a [T], key: fn(&T) -> &str, maximum: usize) -> Self {
        let mut order = Vec::new();
        // An optimization must not add allocation refusal or impose an ordering
        // on callers. Keep the original exact linear comparison if unavailable.
        if values.len() >= 16
            && values.len() <= maximum
            && order.try_reserve_exact(values.len()).is_ok()
        {
            order.extend(0..values.len());
            order.sort_unstable_by(|a, b| key(&values[*a]).cmp(key(&values[*b])));
        }
        Self { values, key, order }
    }

    pub(super) fn contains(&self, value: &T) -> bool {
        if self.order.is_empty() {
            return self.values.contains(value);
        }
        let key = (self.key)(value);
        let first = self
            .order
            .partition_point(|i| (self.key)(&self.values[*i]) < key);
        self.order[first..]
            .iter()
            .take_while(|i| (self.key)(&self.values[**i]) == key)
            .any(|i| self.values[*i] == *value)
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use alloc::{format, string::String};
    use core::cell::Cell;

    #[derive(Clone)]
    struct Record<'a> {
        key: String,
        content: usize,
        comparisons: &'a Cell<usize>,
    }
    impl PartialEq for Record<'_> {
        fn eq(&self, other: &Self) -> bool {
            self.comparisons.set(self.comparisons.get() + 1);
            self.key == other.key && self.content == other.content
        }
    }

    #[test]
    fn indexed_membership_preserves_linear_equality_under_collisions_and_reordering() {
        let comparisons = Cell::new(0);
        for length in [0, 1, 15, 16, 255, 1024] {
            let mut records: Vec<_> = (0..length)
                .map(|i| Record {
                    key: format!("subject/{}", i % 11),
                    content: i,
                    comparisons: &comparisons,
                })
                .collect();
            records.reverse();
            if length > 1 {
                records.push(records[0].clone());
            }
            for maximum in [0, 2048] {
                let indexed = Lookup::new(&records, |r| &r.key, maximum);
                for i in 0..length + 2 {
                    let mut probe = Record {
                        key: format!("subject/{}", i % 11),
                        content: i,
                        comparisons: &comparisons,
                    };
                    assert_eq!(indexed.contains(&probe), records.contains(&probe));
                    probe.key.push_str("foreign");
                    assert_eq!(indexed.contains(&probe), records.contains(&probe));
                }
            }
        }
    }

    #[test]
    fn full_record_comparisons_drop_for_distinct_subjects() {
        let comparisons = Cell::new(0);
        let records: Vec<_> = (0..1024)
            .map(|i| Record {
                key: format!("subject/{i}"),
                content: i,
                comparisons: &comparisons,
            })
            .collect();
        for record in &records {
            assert!(records.contains(record));
        }
        let linear = comparisons.replace(0);
        let indexed = Lookup::new(&records, |r| &r.key, 1024);
        for record in &records {
            assert!(indexed.contains(record));
        }
        assert_eq!(linear, 1024 * 1025 / 2);
        assert_eq!(comparisons.get(), 1024);
    }

    /// A bounded CPU-only screen over a retained actual Face. Includes index
    /// construction each turn; does not measure the emitter or protected entry.
    #[test]
    #[ignore = "requires an explicitly supplied retained Face JSON fixture"]
    fn retained_face_membership_cpu_screen() {
        use std::{hint::black_box, time::Instant};
        let path = std::env::var("CONDUIT_FACE_LOOKUP_FIXTURE").unwrap();
        let face: conduit_face::Face =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        macro_rules! measure {
            ($field:ident, $key:ident, $max:ident) => {{
                let start = Instant::now();
                for _ in 0..1000 {
                    for record in &face.$field {
                        assert!(black_box(&face.$field).contains(black_box(record)));
                    }
                }
                let baseline = start.elapsed();
                let start = Instant::now();
                for _ in 0..1000 {
                    let index =
                        Lookup::new(black_box(&face.$field), |r| &r.$key, conduit_face::$max);
                    for record in &face.$field {
                        assert!(index.contains(black_box(record)));
                    }
                }
                std::println!(
                    "membership field={} records={} iterations=1000 baseline_ns={} indexed_ns={}",
                    stringify!($field),
                    face.$field.len(),
                    baseline.as_nanos(),
                    start.elapsed().as_nanos()
                );
            }};
        }
        measure!(subjects, identity, MAX_FACE_SUBJECTS);
        measure!(relationships, source, MAX_FACE_RELATIONSHIPS);
        measure!(properties, subject, MAX_FACE_PROPERTIES);
        measure!(text, subject, MAX_FACE_TEXT_ITEMS);
        measure!(disclosures, subject, MAX_FACE_DISCLOSURES);
    }
}
