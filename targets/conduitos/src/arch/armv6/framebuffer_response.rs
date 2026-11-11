//! Validate firmware observations before granting any framebuffer write access.
use crate::display::DisplayFormat;
#[cfg(test)]
pub(crate) fn validate(
    w: &[u32; 40],
    width: u32,
    height: u32,
) -> Result<(u32, u32, DisplayFormat), &'static str> {
    validate_in_window(w, width, height, 0x1c00_0000, 0x2000_0000)
}
pub(crate) fn validate_in_window(
    w: &[u32; 40],
    width: u32,
    height: u32,
    scanout_start: u32,
    scanout_end: u32,
) -> Result<(u32, u32, DisplayFormat), &'static str> {
    for (index, bytes) in [
        (2, 8),
        (7, 8),
        (12, 8),
        (17, 4),
        (21, 4),
        (25, 8),
        (30, 4),
        (34, 8),
    ] {
        if w[index + 2] != 0x8000_0000 | bytes {
            return Err("bcm2835-framebuffer-tag-invalid");
        }
    }
    if w[5] != width
        || w[6] != height
        || w[10] != width
        || w[11] != height
        || w[15] != 0
        || w[16] != 0
        || w[20] != 32
        || w[24] > 1
    {
        return Err("bcm2835-framebuffer-format-refused");
    }
    let physical = w[28] & 0x3fff_ffff;
    let bytes = w[29];
    let gpu_start = w[37];
    let gpu_end = gpu_start
        .checked_add(w[38])
        .ok_or("bcm2835-gpu-range-invalid")?;
    if physical == 0
        || physical & 4095 != 0
        || bytes == 0
        || bytes > (8 * 1024 * 1024)
        || physical < scanout_start // Must remain inside the CPU adapter's uncached window.
        || physical < gpu_start
        || physical
            .checked_add(bytes)
            .is_none_or(|end| end > gpu_end || end > scanout_end)
    {
        return Err("bcm2835-framebuffer-reservation-invalid");
    }
    let format = DisplayFormat {
        width,
        height,
        pitch: w[33],
        bits_per_pixel: 32,
        red_shift: if w[24] == 1 { 0 } else { 16 },
        green_shift: 8,
        blue_shift: if w[24] == 1 { 16 } else { 0 },
    };
    let format = format.validate().map_err(|e| e.as_str())?;
    if format.byte_len().map_err(|e| e.as_str())? > bytes as usize {
        return Err("bcm2835-framebuffer-too-small");
    }
    Ok((physical, bytes, format))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn response() -> [u32; 40] {
        let mut w = [0; 40];
        for (i, n) in [
            (2, 8),
            (7, 8),
            (12, 8),
            (17, 4),
            (21, 4),
            (25, 8),
            (30, 4),
            (34, 8),
        ] {
            w[i + 2] = 0x8000_0000 | n;
        }
        w[5] = 800;
        w[6] = 600;
        w[10] = 800;
        w[11] = 600;
        w[20] = 32;
        w[24] = 1;
        w[28] = 0x5c00_0000;
        w[29] = 800 * 600 * 4;
        w[33] = 800 * 4;
        w[37] = 0x1c00_0000;
        w[38] = 0x0400_0000;
        w
    }
    #[test]
    fn exact_firmware_owned_rgb_buffer_is_admitted() {
        let (a, n, f) = validate(&response(), 800, 600).unwrap();
        assert_eq!(a, 0x1c00_0000);
        assert_eq!(n, 1_920_000);
        assert_eq!((f.red_shift, f.blue_shift), (0, 16));
    }
    #[test]
    fn observed_panel_geometry_replaces_the_previous_fixed_size() {
        let mut w = response();
        w[5] = 1024;
        w[10] = 1024;
        w[29] = 1024 * 600 * 4;
        w[33] = 4096;
        let (_, _, format) = validate(&w, 1024, 600).unwrap();
        assert_eq!((format.width, format.height), (1024, 600));
        assert!(validate(&w, 800, 600).is_err());
    }
    #[test]
    fn three_b_plus_scanout_must_match_both_firmware_and_cpu_mapping() {
        let mut w = response();
        w[28] = 0xfc00_0000; // GPU bus alias of physical 0x3c000000.
        w[37] = 0x3c00_0000;
        assert!(validate(&w, 800, 600).is_err());
        let (physical, _, _) = validate_in_window(&w, 800, 600, 0x3000_0000, 0x3f00_0000).unwrap();
        assert_eq!(physical, 0x3c00_0000);
        // A firmware claim cannot grant writes into Device MMIO or cached RAM.
        for address in [0x2f00_0000, 0x3eff_f000, 0x3f00_0000] {
            w[28] = address;
            w[37] = address;
            assert!(validate_in_window(&w, 800, 600, 0x3000_0000, 0x3f00_0000).is_err());
        }
    }
    #[test]
    fn malformed_response_cannot_grant_ram_or_invalid_pixel_access() {
        for (i, v) in [
            (4, 8),
            (20, 16),
            (24, 2),
            (28, 0x4000_8000),
            (29, 16),
            (33, 1),
            (38, u32::MAX),
        ] {
            let mut w = response();
            w[i] = v;
            assert!(validate(&w, 800, 600).is_err(), "word {i}");
        }
    }
}
