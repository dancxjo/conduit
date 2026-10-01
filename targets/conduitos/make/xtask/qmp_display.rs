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
        .as_chunks::<3>()
        .0
        .iter()
        .zip(frame.data_mut().as_chunks_mut::<4>().0.iter_mut())
    {
        target[..3].copy_from_slice(source);
        target[3] = 255;
    }
    Ok(frame)
}

pub(super) fn require_content(frame: &Pixmap) -> Result<(), ConduitosError> {
    let first = &frame.data()[..4];
    if frame
        .data()
        .as_chunks::<4>()
        .0
        .iter()
        .all(|pixel| pixel == first)
    {
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
    reader: &mut super::qmp::Reader,
    directory: &std::path::Path,
    checkpoint: &str,
) -> Result<(serde_json::Value, Option<ConduitosError>), ConduitosError> {
    use sha2::{Digest, Sha256};
    use std::fs;
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
    let frame = read_complete_frame(
        &ppm,
        std::time::Instant::now() + std::time::Duration::from_secs(2),
    )?;
    let encoded = frame.encode_png().map_err(|error| {
        ConduitosError::refusal("qemu-display-encode-failed", error.to_string())
    })?;
    fs::write(&png, &encoded).map_err(io_error)?;
    fs::remove_file(ppm).map_err(io_error)?;
    let health_refusal = require_content(&frame).err();
    Ok((
        serde_json::json!({"checkpoint":checkpoint,"png":png.file_name().unwrap().to_string_lossy(),
        "width":frame.width(),"height":frame.height(),
        "non_background_pixels":frame.data().as_chunks::<4>().0.iter().filter(|pixel|*pixel != &frame.data()[..4]).count(),
        "pixel_format":"RGBA8","png_bytes":encoded.len(),
        "png_sha256":format!("{:x}",Sha256::digest(&encoded)),"pixel_sha256":format!("{:x}",Sha256::digest(frame.data()))}),
        health_refusal,
    ))
}

fn read_complete_frame(
    path: &std::path::Path,
    deadline: std::time::Instant,
) -> Result<Pixmap, ConduitosError> {
    use std::{
        fs,
        io::Read,
        time::{Duration, Instant},
    };
    loop {
        let result = (|| {
            let mut bytes = Vec::new();
            fs::File::open(path)
                .map_err(|error| {
                    ConduitosError::refusal(
                        if error.kind() == std::io::ErrorKind::NotFound {
                            "qemu-display-unavailable"
                        } else {
                            "qemu-display-artifact-io"
                        },
                        error.to_string(),
                    )
                })?
                .take((MAXIMUM_FRAME_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
                .map_err(|error| {
                    ConduitosError::refusal("qemu-display-artifact-io", error.to_string())
                })?;
            decode(&bytes)
        })();
        match result {
            Ok(frame) => return Ok(frame),
            Err(error)
                if Instant::now() < deadline
                    && matches!(
                        error.reason,
                        "qemu-display-unavailable" | "qemu-display-malformed-ppm"
                    ) =>
            {
                // File publication can lag the acknowledged command. This polls bytes,
                // never reissues screendump or performs another guest action.
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => return Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unavailable_file_and_malformed_file_remain_distinct_at_deadline() {
        let path =
            std::env::temp_dir().join(format!("conduit-ppm-deadline-{}", std::process::id()));
        assert_eq!(
            read_complete_frame(&path, std::time::Instant::now())
                .err()
                .unwrap()
                .reason,
            "qemu-display-unavailable"
        );
        std::fs::write(&path, b"P6\n1 1\n255\nxx").unwrap();
        assert_eq!(
            read_complete_frame(&path, std::time::Instant::now())
                .err()
                .unwrap()
                .reason,
            "qemu-display-malformed-ppm"
        );
        std::fs::remove_file(path).unwrap();
    }

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
