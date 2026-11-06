//! Bounded display-identification facts shared by firmware scanout adapters.
//! Interlaced and extension-only timings are not inferred from a base block.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreferredTiming {
    pub width: u32,
    pub height: u32,
    pub pixel_clock_khz: u32,
    pub hsync_start: u16,
    pub hsync_end: u16,
    pub htotal: u16,
    pub vsync_start: u16,
    pub vsync_end: u16,
    pub vtotal: u16,
    pub positive_hsync: bool,
    pub positive_vsync: bool,
}
pub fn preferred_timing(block: &[u8; 128]) -> Option<PreferredTiming> {
    if block[..8] != [0, 255, 255, 255, 255, 255, 255, 0]
        || block.iter().fold(0_u8, |sum, byte| sum.wrapping_add(*byte)) != 0
        || block[18] != 1
        || block[24] & 2 == 0
    {
        return None;
    }
    let timing = &block[54..72];
    if u16::from_le_bytes([timing[0], timing[1]]) == 0 || timing[17] & 0x80 != 0 {
        return None;
    }
    let width = u32::from(timing[2]) | (u32::from(timing[4] & 0xf0) << 4);
    let height = u32::from(timing[5]) | (u32::from(timing[7] & 0xf0) << 4);
    let hblank = u32::from(timing[3]) | (u32::from(timing[4] & 15) << 8);
    let vblank = u32::from(timing[6]) | (u32::from(timing[7] & 15) << 8);
    let hfront = u32::from(timing[8]) | (u32::from(timing[11] >> 6) << 8);
    let hpulse = u32::from(timing[9]) | (u32::from((timing[11] >> 4) & 3) << 8);
    let vfront = u32::from(timing[10] >> 4) | (u32::from((timing[11] >> 2) & 3) << 4);
    let vpulse = u32::from(timing[10] & 15) | (u32::from(timing[11] & 3) << 4);
    if width == 0
        || height == 0
        || hpulse == 0
        || vpulse == 0
        || hfront + hpulse > hblank
        || vfront + vpulse > vblank
        || timing[17] & 0x18 != 0x18
    {
        return None;
    }
    Some(PreferredTiming {
        width,
        height,
        pixel_clock_khz: u32::from(u16::from_le_bytes([timing[0], timing[1]])) * 10,
        hsync_start: (width + hfront) as u16,
        hsync_end: (width + hfront + hpulse) as u16,
        htotal: (width + hblank) as u16,
        vsync_start: (height + vfront) as u16,
        vsync_end: (height + vfront + vpulse) as u16,
        vtotal: (height + vblank) as u16,
        positive_hsync: timing[17] & 2 != 0,
        positive_vsync: timing[17] & 4 != 0,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn checksum(block: &mut [u8; 128]) {
        block[127] = 0;
        block[127] = 0_u8.wrapping_sub(block.iter().fold(0_u8, |s, b| s.wrapping_add(*b)));
    }
    fn panel() -> [u8; 128] {
        let mut block = [0; 128];
        block[..8].copy_from_slice(&[0, 255, 255, 255, 255, 255, 255, 0]);
        block[18] = 1;
        block[19] = 3;
        block[24] = 2;
        block[54..56].copy_from_slice(&5120_u16.to_le_bytes());
        block[56] = 0;
        block[58] = 0x40;
        block[59] = 0x58;
        block[61] = 0x20;
        block[57] = 0x40;
        block[58] |= 1; // 320 horizontal blanking.
        block[60] = 35;
        block[62] = 24;
        block[63] = 136;
        block[64] = 0x36;
        block[71] = 0x1e;
        checksum(&mut block);
        block
    }
    #[test]
    fn checked_preferred_timing_preserves_nonstandard_panel_dimensions() {
        let timing = preferred_timing(&panel()).unwrap();
        assert_eq!((timing.width, timing.height), (1024, 600));
        assert_eq!(timing.pixel_clock_khz, 51200);
        assert_eq!(
            (timing.hsync_start, timing.hsync_end, timing.htotal),
            (1048, 1184, 1344)
        );
        assert_eq!(
            (timing.vsync_start, timing.vsync_end, timing.vtotal),
            (603, 609, 635)
        );
        assert!(timing.positive_hsync && timing.positive_vsync);
    }
    #[test]
    fn corrupt_unpreferred_missing_and_interlaced_timings_are_not_native_claims() {
        let mut bad = panel();
        bad[0] = 1;
        checksum(&mut bad);
        assert_eq!(preferred_timing(&bad), None);
        bad = panel();
        bad[10] ^= 1;
        assert_eq!(preferred_timing(&bad), None);
        for (index, value) in [(24, 0), (55, 0), (71, 0x80), (18, 2), (63, 0), (58, 0x40)] {
            bad = panel();
            bad[index] = value;
            checksum(&mut bad);
            assert_eq!(preferred_timing(&bad), None);
        }
    }
}
