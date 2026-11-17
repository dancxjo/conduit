//! Checked internal-DRAM construction policy for this ESP32 BSP only.
//! The requested IMAGE total is split, never enlarged by discovered free RAM.
pub const SECONDARY_BYTES: u64 = 98_304;
// The additional 8 KiB funds the retained continuation prefix explicitly.
pub const PRIMARY_BYTES: u64 = 20_480;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeapLayout {
    pub secondary: u64,
    pub primary: u64,
}
pub fn checked(total: u64) -> Option<HeapLayout> {
    if total == 0 || total > SECONDARY_BYTES + PRIMARY_BYTES || total % 16 != 0 {
        return None;
    }
    let secondary = total.min(SECONDARY_BYTES);
    Some(HeapLayout {
        secondary,
        primary: total - secondary,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_total_is_partitioned_without_expansion() {
        assert_eq!(
            checked(98_304),
            Some(HeapLayout {
                secondary: 98_304,
                primary: 0
            })
        );
        assert_eq!(
            checked(110_592),
            Some(HeapLayout {
                secondary: 98_304,
                primary: 12_288
            })
        );
        assert_eq!(
            checked(102_400),
            Some(HeapLayout {
                secondary: 98_304,
                primary: 4_096
            })
        );
        assert_eq!(
            checked(118_784),
            Some(HeapLayout {
                secondary: 98_304,
                primary: 20_480
            })
        );
        for total in [0, 98_305, 118_800, u64::MAX] {
            assert_eq!(checked(total), None);
        }
    }
}
