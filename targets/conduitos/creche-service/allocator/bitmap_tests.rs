use super::*;

fn reference_find(
    bitmap: &[u64],
    heap_start: usize,
    mut index: usize,
    end: usize,
    needed: usize,
    alignment: usize,
) -> Option<usize> {
    while index.checked_add(needed).is_some_and(|n| n <= end) {
        let address = (heap_start + index * UNIT).checked_add(alignment - 1)? & !(alignment - 1);
        index = (address - heap_start) / UNIT;
        if index.checked_add(needed).is_none_or(|n| n > end) {
            return None;
        }
        let mut cursor = index;
        while cursor < index + needed && bitmap[cursor / 64] & (1 << (cursor % 64)) == 0 {
            cursor += 1;
        }
        if cursor == index + needed {
            return Some(index);
        }
        index = cursor + 1;
    }
    None
}

#[test]
fn mark_matches_independent_per_bit_reference_across_word_edges() {
    for original in [0, u64::MAX, 0x5555_5555_5555_5555, 0x89ab_cdef_0123_4567] {
        for start in 0..=192 {
            for end in start..=192 {
                for used in [false, true] {
                    let mut expected = [original; 3];
                    for bit in start..end {
                        if used {
                            expected[bit / 64] |= 1 << (bit % 64);
                        } else {
                            expected[bit / 64] &= !(1 << (bit % 64));
                        }
                    }
                    let mut actual = [original; 3];
                    assert!(mark(&mut actual, start, end, used));
                    assert_eq!(actual, expected, "{start}..{end}, used={used}");
                }
            }
        }
    }
}

#[test]
fn first_fit_and_alignment_match_reference_for_every_small_bitmap() {
    for bits in 0..=u16::MAX {
        let bitmap = [u64::from(bits)];
        for (start, end) in [(0, 16), (1, 16), (7, 16), (8, 12), (16, 16)] {
            for needed in [1, 2, 3, 7, 16, 17] {
                for alignment in [UNIT, 64, 128, 4096] {
                    assert_eq!(
                        find(&bitmap, 0x640000, start, end, needed, alignment),
                        reference_find(&bitmap, 0x640000, start, end, needed, alignment),
                        "bits={bits:#x} start={start} end={end} needed={needed} alignment={alignment}"
                    );
                }
            }
        }
    }
}

#[test]
fn full_admitted_heap_wrap_exhaustion_reuse_and_no_cross_boundary_run() {
    const UNITS: usize = conduitos_creche_core::memory::HEAP_BYTES / UNIT;
    let mut bitmap = [0; UNITS / 64];
    let heap = conduitos_creche_core::memory::HEAP_START;
    let needed = 128 * 1024 / UNIT;
    let mut cursor = 0;
    for expected in (0..UNITS).step_by(needed) {
        let needed = needed.min(UNITS - expected);
        let found = find(&bitmap, heap, cursor, UNITS, needed, UNIT)
            .or_else(|| find(&bitmap, heap, 0, cursor, needed, UNIT))
            .unwrap();
        assert_eq!(found, expected);
        mark(&mut bitmap, found, found + needed, true);
        cursor = found + needed;
    }
    assert!(bitmap.iter().all(|&word| word == u64::MAX));
    assert_eq!(find(&bitmap, heap, 0, UNITS, 1, UNIT), None);
    mark(&mut bitmap, 65, 65 + needed, false);
    assert_eq!(
        find(&bitmap, heap, cursor, UNITS, needed, UNIT)
            .or_else(|| find(&bitmap, heap, 0, cursor, needed, UNIT)),
        Some(65)
    );
    assert_eq!(find(&bitmap, heap, 0, UNITS, needed, 4096), None);
    mark(&mut bitmap, 0, 1, false);
    mark(&mut bitmap, UNITS - 1, UNITS, false);
    assert_eq!(find(&bitmap, heap, UNITS - 1, UNITS, 2, UNIT), None);
    assert_eq!(find(&bitmap, heap, 0, UNITS, 0, UNIT), None);
    assert_eq!(find(&bitmap, heap, 0, UNITS + 1, 1, UNIT), None);
    assert_eq!(find(&bitmap, heap, 0, UNITS, 1, 0), None);
}

#[test]
fn cross_word_fragmented_search_matches_reference() {
    let mut bitmap = [u64::MAX; 5];
    for (start, end) in [(60, 69), (127, 192), (255, 319)] {
        mark(&mut bitmap, start, end, false);
    }
    for start in 0..=320 {
        for needed in 1..=70 {
            for alignment in [UNIT, 64, 128, 256, 4096] {
                assert_eq!(
                    find(&bitmap, 0x640000, start, 320, needed, alignment),
                    reference_find(&bitmap, 0x640000, start, 320, needed, alignment)
                );
            }
        }
    }
}

#[test]
fn invalid_marks_refuse_without_touching_neighboring_units() {
    let mut bitmap = [0x1234_5678_9abc_def0; 2];
    let original = bitmap;
    for (start, end) in [(65, 64), (0, 129), (128, 129), (usize::MAX, usize::MAX)] {
        assert!(!mark(&mut bitmap, start, end, true));
        assert_eq!(bitmap, original);
    }
    assert!(mark(&mut bitmap, 128, 128, false));
    assert_eq!(bitmap, original);
    assert!(mark(&mut [], 0, 0, false));
    assert!(!mark(&mut [], 0, 1, true));
    assert_eq!(find(&[], 0x640000, 0, 0, 1, UNIT), None);
}

#[test]
fn large_fragmented_ranges_and_shifted_heap_match_original() {
    let mut seed = 0x986a_234b_eda1_375du64;
    for _ in 0..200 {
        let mut words = [0u64; 8];
        for word in &mut words {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            *word = seed & (seed >> 1) & (seed >> 2) & (seed >> 3);
        }
        for heap in [0x640000, 0x640020, 0x640060] {
            for start in [0, 1, 31, 63, 64, 65, 127, 200, 400] {
                for end in [401, 448, 511, 512] {
                    if start > end {
                        continue;
                    }
                    for needed in [64, 65, 66, 95, 127, 128, 129, 255] {
                        for alignment in [32, 64, 128, 256, 4096] {
                            assert_eq!(
                                find(&words, heap, start, end, needed, alignment),
                                reference_find(&words, heap, start, end, needed, alignment)
                            );
                        }
                    }
                }
            }
        }
    }
}
