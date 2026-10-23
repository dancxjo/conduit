//! Pure predicate shared with the actual IRQ save-frame observation.
pub(super) fn canary_range(words: &[u64; 15], flags: u64) -> bool {
    let expected = [
        115, 114, 113, 112, 111, 110, 109, 108, 107, 106, 105, 104, 103, 0, 101,
    ];
    words.iter().zip(expected).enumerate().all(|(i, (v, e))| {
        if i == 13 {
            (1..=1_000_000).contains(v)
        } else {
            *v == e
        }
    }) && flags & (1 << 10) != 0
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_live_register_and_df_is_required() {
        let good = [
            115, 114, 113, 112, 111, 110, 109, 108, 107, 106, 105, 104, 103, 500_000, 101,
        ];
        assert!(canary_range(&good, 0x600));
        assert!(!canary_range(&good, 0x200));
        for index in 0..15 {
            let mut bad = good;
            bad[index] = 0;
            assert!(!canary_range(&bad, 0x600), "register{index}");
        }
        let mut beyond = good;
        beyond[13] = 1_000_001;
        assert!(!canary_range(&beyond, 0x600));
    }
}
