//! Bounded dependency-free PNG encoding for retained native renderer pixels.

use std::io::Write;
use std::path::Path;

const MAX_DIMENSION: usize = 2_048;
const DEFLATE_BLOCK_BYTES: usize = u16::MAX as usize;

pub(super) fn write_rgb_png(
    path: &Path,
    pixels: &[u32],
    width: usize,
    height: usize,
) -> Result<(), String> {
    if width == 0
        || height == 0
        || width > MAX_DIMENSION
        || height > MAX_DIMENSION
        || pixels.len() != width.checked_mul(height).ok_or("PNG extent overflowed")?
    {
        return Err("PNG dimensions or pixel extent are outside the bounded profile".into());
    }
    let row_bytes = width.checked_mul(3).ok_or("PNG row overflowed")?;
    let raw_capacity = height
        .checked_mul(row_bytes.checked_add(1).ok_or("PNG row overflowed")?)
        .ok_or("PNG image overflowed")?;
    let mut raw = Vec::with_capacity(raw_capacity);
    for row in pixels.chunks_exact(width) {
        raw.push(0);
        for pixel in row {
            raw.extend_from_slice(&[
                ((pixel >> 16) & 0xff) as u8,
                ((pixel >> 8) & 0xff) as u8,
                (pixel & 0xff) as u8,
            ]);
        }
    }

    let mut encoded = Vec::with_capacity(raw.len().saturating_add(256));
    encoded.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(
        &u32::try_from(width)
            .map_err(|_| "PNG width overflowed")?
            .to_be_bytes(),
    );
    ihdr.extend_from_slice(
        &u32::try_from(height)
            .map_err(|_| "PNG height overflowed")?
            .to_be_bytes(),
    );
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    append_chunk(&mut encoded, b"IHDR", &ihdr)?;

    let mut zlib = Vec::with_capacity(
        raw.len()
            .saturating_add(raw.len() / DEFLATE_BLOCK_BYTES * 5 + 8),
    );
    zlib.extend_from_slice(&[0x78, 0x01]);
    let block_count = raw.len().div_ceil(DEFLATE_BLOCK_BYTES);
    for (index, block) in raw.chunks(DEFLATE_BLOCK_BYTES).enumerate() {
        zlib.push(u8::from(index + 1 == block_count));
        let length = u16::try_from(block.len()).map_err(|_| "PNG block overflowed")?;
        zlib.extend_from_slice(&length.to_le_bytes());
        zlib.extend_from_slice(&(!length).to_le_bytes());
        zlib.extend_from_slice(block);
    }
    zlib.extend_from_slice(&adler32(&raw).to_be_bytes());
    append_chunk(&mut encoded, b"IDAT", &zlib)?;
    append_chunk(&mut encoded, b"IEND", &[])?;

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("create {}: {error}", path.display()))?;
    file.write_all(&encoded)
        .map_err(|error| format!("write {}: {error}", path.display()))
}

fn append_chunk(output: &mut Vec<u8>, kind: &[u8; 4], payload: &[u8]) -> Result<(), String> {
    output.extend_from_slice(
        &u32::try_from(payload.len())
            .map_err(|_| "PNG chunk overflowed")?
            .to_be_bytes(),
    );
    output.extend_from_slice(kind);
    output.extend_from_slice(payload);
    let start = output.len() - payload.len() - kind.len();
    output.extend_from_slice(&crc32(&output[start..]).to_be_bytes());
    Ok(())
}

fn adler32(bytes: &[u8]) -> u32 {
    let (mut a, mut b) = (1_u32, 0_u32);
    for byte in bytes {
        a = (a + u32::from(*byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    (b << 16) | a
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & (0_u32.wrapping_sub(crc & 1)));
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_encoder_writes_one_new_truecolor_png() {
        let path = std::env::temp_dir().join(format!(
            "conduit-native-png-{}-{}.png",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = std::fs::remove_file(&path);
        write_rgb_png(&path, &[0xff0000, 0x00ff00, 0x0000ff, 0xffffff], 2, 2).unwrap();
        let encoded = std::fs::read(&path).unwrap();
        assert_eq!(&encoded[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(&encoded[12..16], b"IHDR");
        assert!(encoded.windows(4).any(|window| window == b"IDAT"));
        assert_eq!(&encoded[encoded.len() - 8..encoded.len() - 4], b"IEND");
        assert!(write_rgb_png(&path, &[0], 1, 1).is_err());
        std::fs::remove_file(path).unwrap();
    }
}
