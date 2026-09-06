//! Bounded QEMU P6 display decoding. Pixels are visual evidence only.
use super::ConduitosError;
use resvg::tiny_skia::Pixmap;

pub(super) const MAXIMUM_FRAME_BYTES: usize = 4096 * 2160 * 3 + 256;

pub(super) fn decode(bytes: &[u8]) -> Result<Pixmap, ConduitosError> {
    let refuse = |detail| ConduitosError::refusal("qemu-display-malformed-ppm", detail);
    if bytes.len() > MAXIMUM_FRAME_BYTES {
        return Err(ConduitosError::refusal(
            "qemu-display-frame-bound",
            "PPM exceeds admitted display envelope",
        ));
    }
    let mut offset = 0;
    let mut token = || -> Result<&[u8], ConduitosError> {
        loop {
            while bytes.get(offset).is_some_and(u8::is_ascii_whitespace) {
                offset += 1;
            }
            if bytes.get(offset) != Some(&b'#') {
                break;
            }
            while bytes.get(offset).is_some_and(|byte| *byte != b'\n') {
                offset += 1;
            }
        }
        let start = offset;
        while bytes
            .get(offset)
            .is_some_and(|byte| !byte.is_ascii_whitespace())
        {
            offset += 1;
        }
        if start == offset || offset > 256 {
            return Err(refuse("invalid bounded PPM header"));
        }
        Ok(&bytes[start..offset])
    };
    if token()? != b"P6" {
        return Err(refuse("expected binary RGB P6"));
    }
    let number = |value: &[u8]| -> Result<u32, ConduitosError> {
        std::str::from_utf8(value)
            .ok()
            .and_then(|text| text.parse().ok())
            .ok_or_else(|| refuse("invalid PPM number"))
    };
    let width = number(token()?)?;
    let height = number(token()?)?;
    if number(token()?)? != 255 {
        return Err(refuse("expected 8-bit RGB channels"));
    }
    if width == 0 || height == 0 || width > 4096 || height > 2160 {
        return Err(ConduitosError::refusal(
            "qemu-display-dimensions",
            "display is outside admitted viewport bounds",
        ));
    }
    if !bytes.get(offset).is_some_and(u8::is_ascii_whitespace) {
        return Err(refuse("missing raster delimiter"));
    }
    // Consume exactly one separator: initial raster bytes may themselves be whitespace.
    offset += 1;
    let rgb = &bytes[offset..];
    if rgb.len() != width as usize * height as usize * 3 {
        return Err(refuse("partial or excess raster bytes"));
    }
    let mut frame = Pixmap::new(width, height).ok_or_else(|| {
        ConduitosError::refusal(
            "qemu-display-encode-failed",
            "could not allocate bounded frame",
        )
    })?;
    for (source, target) in rgb
        .chunks_exact(3)
        .zip(frame.data_mut().chunks_exact_mut(4))
    {
        target[..3].copy_from_slice(source);
        target[3] = 255;
    }
    Ok(frame)
}

pub(super) fn require_content(frame: &Pixmap) -> Result<(), ConduitosError> {
    let first = &frame.data()[..4];
    if frame.data().chunks_exact(4).all(|pixel| pixel == first) {
        return Err(ConduitosError::refusal(
            "qemu-display-uniform-frame",
            "display contains one uniform color",
        ));
    }
    Ok(())
}

/// Capture uses the same negotiated QMP channel as keyboard input.
pub(super) fn capture(
    stream: &mut std::os::unix::net::UnixStream,
    reader: &mut std::io::BufReader<std::os::unix::net::UnixStream>,
    directory: &std::path::Path,
    checkpoint: &str,
) -> Result<serde_json::Value, ConduitosError> {
    use sha2::{Digest, Sha256};
    use std::{fs, io::Read};
    if checkpoint.is_empty()
        || checkpoint.len() > 64
        || !checkpoint
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'-')
    {
        return Err(ConduitosError::refusal(
            "qemu-display-checkpoint-invalid",
            "invalid checkpoint path component",
        ));
    }
    let io_error = |error: std::io::Error| {
        ConduitosError::refusal("qemu-display-artifact-io", error.to_string())
    };
    fs::create_dir_all(directory).map_err(io_error)?;
    let directory = fs::canonicalize(directory).map_err(io_error)?;
    let ppm = directory.join(format!("{checkpoint}.ppm"));
    let png = directory.join(format!("{checkpoint}.png"));
    if ppm.exists() {
        fs::remove_file(&ppm).map_err(io_error)?;
    }
    let command = serde_json::json!({"execute":"screendump","arguments":{"filename":ppm}});
    super::qmp::request(stream, reader, command.to_string().as_bytes(), checkpoint)?;
    let mut bytes = Vec::new();
    fs::File::open(&ppm)
        .map_err(io_error)?
        .take((MAXIMUM_FRAME_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    let frame = decode(&bytes)?;
    let encoded = frame.encode_png().map_err(|error| {
        ConduitosError::refusal("qemu-display-encode-failed", error.to_string())
    })?;
    fs::write(&png, &encoded).map_err(io_error)?;
    fs::remove_file(ppm).map_err(io_error)?;
    require_content(&frame)?;
    Ok(
        serde_json::json!({"checkpoint":checkpoint,"png":png.file_name().unwrap().to_string_lossy(),
        "width":frame.width(),"height":frame.height(),"pixel_format":"RGBA8","png_bytes":encoded.len(),
        "png_sha256":format!("{:x}",Sha256::digest(&encoded)),"pixel_sha256":format!("{:x}",Sha256::digest(frame.data()))}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn binary_pixels_survive_png_without_whitespace_loss() {
        let input = b"P6\n# QEMU\n2 1\n255\n\n\r\t\0\x80\xff";
        let frame = decode(input).unwrap();
        require_content(&frame).unwrap();
        let encoded = frame.encode_png().unwrap();
        let restored = Pixmap::decode_png(&encoded).unwrap();
        assert_eq!(restored.data(), &[10, 13, 9, 255, 0, 128, 255, 255]);
    }
    #[test]
    fn partial_excess_and_invalid_frames_refuse() {
        for input in [
            b"P6\n0 1\n255\n".as_slice(),
            b"P6\n1 1\n255\nxx",
            b"P6\n1 1\n255\nxxxx",
            b"P3\n1 1\n255\nxxx",
            b"P6\n4097 1\n255\n",
        ] {
            assert!(decode(input).is_err());
        }
        assert!(require_content(&decode(b"P6\n1 1\n255\nxxx").unwrap()).is_err());
    }
}
