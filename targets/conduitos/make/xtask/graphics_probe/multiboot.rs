//! Pinned Limine 12.5.2 Multiboot-1 observation layout, not a scanout grant.
//! Its explicit reserved u16 puts RGB fields at byte 112, not byte 110.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Framebuffer {
    pub address: u64,
    pub pitch: u32,
    pub width: u32,
    pub height: u32,
    pub bits_per_pixel: u8,
    pub framebuffer_type: u8,
    /// (mask size, shift), matching the portable display description.
    pub red: (u8, u8),
    pub green: (u8, u8),
    pub blue: (u8, u8),
}

fn bytes<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N], &'static str> {
    input
        .get(offset..offset + N)
        .ok_or("multiboot-information-truncated")?
        .try_into()
        .map_err(|_| "multiboot-information-truncated")
}

pub fn observe(magic: u32, input: &[u8]) -> Result<Option<Framebuffer>, &'static str> {
    if magic != 0x2bad_b002 {
        return Err("multiboot-magic-mismatch");
    }
    let flags = u32::from_le_bytes(bytes(input, 0)?);
    if flags & (1 << 12) == 0 {
        return Ok(None);
    }
    if input.len() < 118 {
        return Err("multiboot-information-truncated");
    }
    Ok(Some(Framebuffer {
        address: u64::from_le_bytes(bytes(input, 88)?),
        pitch: u32::from_le_bytes(bytes(input, 96)?),
        width: u32::from_le_bytes(bytes(input, 100)?),
        height: u32::from_le_bytes(bytes(input, 104)?),
        bits_per_pixel: input[108],
        framebuffer_type: input[109],
        red: (input[113], input[112]),
        green: (input[115], input[114]),
        blue: (input[117], input[116]),
    }))
}

// Interpretation, resource admission and pixel access belong to the existing
// display/Host contracts. This parser retains unsupported raw modes as evidence.

#[cfg(test)]
mod tests {
    use super::*;
    fn handoff() -> [u8; 128] {
        let mut input = [0; 128];
        input[..4].copy_from_slice(&0x126d_u32.to_le_bytes());
        input[88..96].copy_from_slice(&0x8000_0000_u64.to_le_bytes());
        input[96..100].copy_from_slice(&5120_u32.to_le_bytes());
        input[100..104].copy_from_slice(&1280_u32.to_le_bytes());
        input[104..108].copy_from_slice(&800_u32.to_le_bytes());
        input[108] = 32;
        input[109] = 1;
        input[112..118].copy_from_slice(&[16, 8, 8, 8, 0, 8]);
        input
    }
    #[test]
    fn decodes_observed_uefi_rgb_handoff_with_pinned_padding() {
        let display = observe(0x2bad_b002, &handoff()).unwrap().unwrap();
        assert_eq!(display.red, (8, 16));
        assert_eq!(display.green, (8, 8));
        assert_eq!(display.blue, (8, 0));
        assert_eq!(
            (
                display.address,
                display.pitch,
                display.width,
                display.height,
                display.bits_per_pixel,
                display.framebuffer_type
            ),
            (0x8000_0000, 5120, 1280, 800, 32, 1)
        );
    }
    #[test]
    fn absence_wrong_magic_and_truncation_remain_distinct() {
        assert_eq!(observe(0, &handoff()), Err("multiboot-magic-mismatch"));
        assert_eq!(observe(0x2bad_b002, &[0; 4]), Ok(None));
        for n in 0..118 {
            assert!(observe(0x2bad_b002, &handoff()[..n]).is_err(), "prefix {n}");
        }
    }
    #[test]
    fn legacy_text_is_observed_without_becoming_linear_graphics() {
        let mut input = handoff();
        input[109] = 2;
        input[108] = 16;
        let text = observe(0x2bad_b002, &input).unwrap().unwrap();
        assert_eq!(text.framebuffer_type, 2);
        assert_eq!(text.bits_per_pixel, 16);
    }
}
