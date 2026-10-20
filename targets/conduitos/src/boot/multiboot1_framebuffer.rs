//! Pinned Limine Multiboot 1 framebuffer facts; no mapping or device authority.

#![cfg_attr(test, allow(dead_code))]

use crate::display::DisplayFormat;

const MAGIC: u32 = 0x2bad_b002;
const FRAMEBUFFER_PRESENT: u32 = 1 << 12;
const RGB_INFORMATION_BYTES: usize = 118;
const MAX_FRAMEBUFFER_BYTES: u64 = 8 * 1024 * 1024;

// The five address fields exist even without flag 16. Limine v12.5.2 reads
// the video fields at offsets 32..48. Text mode preserves the BIOS entrance;
// its UEFI entrance falls back to GOP using these requested dimensions.
pub const VIDEO_HEADER: [u32; 12] = [0x1bad_b002, 4, 0xe452_4ffa, 0, 0, 0, 0, 0, 1, 1280, 800, 32];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FramebufferFacts {
    pub physical_start: u32,
    pub byte_len: u32,
    pub format: DisplayFormat,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FramebufferRefusal {
    WrongMagic,
    TruncatedInformation,
    UnsupportedFormat,
    InvalidExtent,
    AddressOutsideIa32,
}

impl FramebufferRefusal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WrongMagic => "multiboot-framebuffer-wrong-magic",
            Self::TruncatedInformation => "multiboot-framebuffer-information-truncated",
            Self::UnsupportedFormat => "multiboot-framebuffer-format-unsupported",
            Self::InvalidExtent => "multiboot-framebuffer-extent-invalid",
            Self::AddressOutsideIa32 => "multiboot-framebuffer-address-outside-ia32",
        }
    }
}

/// Decode an explicitly bounded bootloader snapshot. Absence and EGA text are
/// observations without an RGB provider; malformed advertised RGB is a refusal.
pub fn decode(magic: u32, bytes: &[u8]) -> Result<Option<FramebufferFacts>, FramebufferRefusal> {
    if magic != MAGIC {
        return Err(FramebufferRefusal::WrongMagic);
    }
    let flags = read_u32(bytes, 0)?;
    if flags & FRAMEBUFFER_PRESENT == 0 {
        return Ok(None);
    }
    if bytes.len() < RGB_INFORMATION_BYTES {
        return Err(FramebufferRefusal::TruncatedInformation);
    }
    if bytes[109] == 2 {
        return Ok(None);
    }
    // Limine's ABI includes a reserved u16 at 110: RGB starts at 112.
    // Reading the packed prose offset 110 silently loses the red mask.
    if bytes[109] != 1 || bytes[113] != 8 || bytes[115] != 8 || bytes[117] != 8 {
        return Err(FramebufferRefusal::UnsupportedFormat);
    }
    let format = DisplayFormat {
        pitch: read_u32(bytes, 96)?,
        width: read_u32(bytes, 100)?,
        height: read_u32(bytes, 104)?,
        bits_per_pixel: bytes[108],
        red_shift: bytes[112],
        green_shift: bytes[114],
        blue_shift: bytes[116],
    };
    format.validate().map_err(|error| match error {
        crate::display::DisplayError::UnsupportedFormat => FramebufferRefusal::UnsupportedFormat,
        _ => FramebufferRefusal::InvalidExtent,
    })?;
    let byte_len = u64::from(format.pitch) * u64::from(format.height);
    if byte_len > MAX_FRAMEBUFFER_BYTES {
        return Err(FramebufferRefusal::InvalidExtent);
    }
    let address = u64::from(read_u32(bytes, 88)?) | (u64::from(read_u32(bytes, 92)?) << 32);
    if address == 0
        || address % 4 != 0
        || address
            .checked_add(byte_len)
            .is_none_or(|end| end > (1_u64 << 32))
    {
        return Err(FramebufferRefusal::AddressOutsideIa32);
    }
    Ok(Some(FramebufferFacts {
        physical_start: address as u32,
        byte_len: byte_len as u32,
        format,
    }))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, FramebufferRefusal> {
    let word = bytes
        .get(offset..offset + 4)
        .ok_or(FramebufferRefusal::TruncatedInformation)?;
    Ok(u32::from_le_bytes(word.try_into().unwrap()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observed_rgb() -> [u8; 128] {
        let mut bytes = [0; 128];
        for (offset, value) in [
            (0, FRAMEBUFFER_PRESENT),
            (88, 0x8000_0000),
            (96, 5120),
            (100, 1280),
            (104, 800),
        ] {
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        bytes[108] = 32;
        bytes[109] = 1;
        bytes[112..118].copy_from_slice(&[16, 8, 8, 8, 0, 8]);
        bytes
    }

    #[test]
    fn pinned_limine_rgb_uses_aligned_union_and_exact_extent() {
        let facts = decode(MAGIC, &observed_rgb()).unwrap().unwrap();
        assert_eq!(facts.physical_start, 0x8000_0000);
        assert_eq!(facts.byte_len, 4_096_000);
        assert_eq!(
            (
                facts.format.red_shift,
                facts.format.green_shift,
                facts.format.blue_shift
            ),
            (16, 8, 0)
        );
        assert_eq!(VIDEO_HEADER.len() * 4, 48);
        assert_eq!(
            VIDEO_HEADER[..3]
                .iter()
                .copied()
                .fold(0_u32, u32::wrapping_add),
            0
        );
        assert_eq!(&VIDEO_HEADER[8..], &[1, 1280, 800, 32]);
    }

    #[test]
    fn absent_and_text_are_not_rgb_providers() {
        assert_eq!(decode(MAGIC, &[0; 4]), Ok(None));
        let mut bytes = observed_rgb();
        bytes[109] = 2;
        assert_eq!(decode(MAGIC, &bytes), Ok(None));
    }

    #[test]
    fn malformed_handoff_refusals_remain_distinct() {
        let bytes = observed_rgb();
        assert_eq!(decode(0, &bytes), Err(FramebufferRefusal::WrongMagic));
        assert_eq!(
            decode(MAGIC, &bytes[..117]),
            Err(FramebufferRefusal::TruncatedInformation)
        );
        let mut bad = bytes;
        bad[113] = 0;
        assert_eq!(
            decode(MAGIC, &bad),
            Err(FramebufferRefusal::UnsupportedFormat)
        );
        bad = bytes;
        bad[92] = 1;
        assert_eq!(
            decode(MAGIC, &bad),
            Err(FramebufferRefusal::AddressOutsideIa32)
        );
        bad = bytes;
        bad[104..108].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(decode(MAGIC, &bad), Err(FramebufferRefusal::InvalidExtent));
    }
}
