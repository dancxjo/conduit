extern crate std;

use super::*;

#[test]
fn search_preserves_first_fit_for_every_small_fragmentation_and_alignment() {
    std::thread::Builder::new().stack_size(16 * 1024 * 1024).spawn(|| {
        let mut arena = ArenaState::new();
        arena.initialize(0x1000, 8 * UNIT).unwrap();
        for occupied in 0_u64..256 {
            arena.allocated[0] = occupied;
            for begin in 0..=8 {
                for end_start in begin..=8 {
                    for needed in 1..=8 {
                        for alignment in [UNIT, 2 * UNIT, 4 * UNIT, 8 * UNIT] {
                            let expected = (begin..end_start).find(|&start| {
                                (arena.start + start * UNIT).is_multiple_of(alignment)
                                    && start + needed <= arena.units
                                    && (start..start + needed).all(|unit| occupied & (1 << unit) == 0)
                            });
                            assert_eq!(arena.find_run(begin, end_start, needed, alignment), expected,
                                "mask {occupied:#x}, starts {begin}..{end_start}, size {needed}, alignment {alignment}");
                        }
                    }
                }
            }
        }
    }).unwrap().join().unwrap();
}

#[test]
fn occupied_words_and_short_cross_word_holes_do_not_hide_the_first_fitting_run() {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            let mut arena = ArenaState::new();
            arena.initialize(0x1000, 193 * UNIT).unwrap();
            arena.mark(0, 128, true);
            assert_eq!(arena.first_free(1, 193), Some(128));
            assert_eq!(arena.first_free(0, 128), None);
            assert_eq!(arena.first_free(193, 193), None);
            arena.mark(63, 65, false);
            assert_eq!(arena.find_run(0, 193, 2, UNIT), Some(63));
            assert_eq!(arena.find_run(0, 193, 2, 2 * UNIT), Some(128));
            assert_eq!(arena.find_run(0, 193, 3, UNIT), Some(128));
        })
        .unwrap()
        .join()
        .unwrap();
}
