//! Bounded framing and result parsing for the hosted local OCR provider.

use conduit_human::{ImageRegion, MAXIMUM_VISIBLE_TEXT_BYTES};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub const MAXIMUM_OCR_ITEMS: usize = 8;
const MAXIMUM_PROVIDER_OUTPUT_BYTES: usize = 64 * 1024;
const MAXIMUM_PROVIDER_DIAGNOSTIC_BYTES: usize = 8 * 1024;
const MAXIMUM_LANGUAGE_BYTES: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OcrCandidate<'a> {
    pub text: &'a str,
    pub region: ImageRegion,
    pub confidence_permille: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OcrProviderRefusal {
    InvalidImage,
    InvalidExecutable,
    InvalidLanguage,
    InvalidProviderOutput,
    OutputCapacity,
    ProviderFailed,
    TimedOut,
}

/// An exact executable-backed Tesseract provider with finite prepared I/O.
pub struct TesseractOcrProvider {
    executable: PathBuf,
    executable_sha256: String,
    language: String,
    timeout: Duration,
    graymap: Vec<u8>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl TesseractOcrProvider {
    pub fn prepare(
        executable: impl AsRef<Path>,
        language: impl Into<String>,
        maximum_pixels: usize,
        timeout: Duration,
    ) -> Result<Self, OcrProviderRefusal> {
        let executable = exact_file(executable.as_ref())?;
        let language = language.into();
        if language.is_empty()
            || language.len() > MAXIMUM_LANGUAGE_BYTES
            || !language
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'+'))
        {
            return Err(OcrProviderRefusal::InvalidLanguage);
        }
        if maximum_pixels == 0 || timeout.is_zero() {
            return Err(OcrProviderRefusal::InvalidImage);
        }
        let graymap_capacity = maximum_pixels
            .checked_add(32)
            .ok_or(OcrProviderRefusal::OutputCapacity)?;
        Ok(Self {
            executable_sha256: digest_file(&executable)?,
            executable,
            language,
            timeout,
            graymap: Vec::with_capacity(graymap_capacity),
            stdout: Vec::with_capacity(MAXIMUM_PROVIDER_OUTPUT_BYTES),
            stderr: Vec::with_capacity(MAXIMUM_PROVIDER_DIAGNOSTIC_BYTES),
        })
    }

    pub fn executable(&self) -> &Path {
        &self.executable
    }

    pub fn executable_sha256(&self) -> &str {
        &self.executable_sha256
    }

    pub fn language(&self) -> &str {
        &self.language
    }

    pub fn recognize(
        &mut self,
        pixels: &[u8],
        width: u16,
        height: u16,
        visit: impl FnMut(OcrCandidate<'_>) -> Result<(), OcrProviderRefusal>,
    ) -> Result<usize, OcrProviderRefusal> {
        encode_graymap(pixels, width, height, &mut self.graymap)?;
        let mut child = Command::new(&self.executable)
            .args(["stdin", "stdout", "-l", &self.language, "tsv"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| OcrProviderRefusal::ProviderFailed)?;
        let stdout = child
            .stdout
            .take()
            .ok_or(OcrProviderRefusal::ProviderFailed)?;
        let stderr = child
            .stderr
            .take()
            .ok_or(OcrProviderRefusal::ProviderFailed)?;
        let stdout_buffer = std::mem::take(&mut self.stdout);
        let stderr_buffer = std::mem::take(&mut self.stderr);
        let stdout_reader = std::thread::spawn(move || {
            bounded_read(stdout, stdout_buffer, MAXIMUM_PROVIDER_OUTPUT_BYTES)
        });
        let stderr_reader = std::thread::spawn(move || {
            bounded_read(stderr, stderr_buffer, MAXIMUM_PROVIDER_DIAGNOSTIC_BYTES)
        });

        let mut stdin = child
            .stdin
            .take()
            .ok_or(OcrProviderRefusal::ProviderFailed)?;
        let input_buffer = std::mem::take(&mut self.graymap);
        let input_writer = std::thread::spawn(move || {
            let result = stdin
                .write_all(&input_buffer)
                .map_err(|_| OcrProviderRefusal::ProviderFailed);
            (input_buffer, result)
        });
        let status = wait_until(&mut child, self.timeout);
        let (input_buffer, wrote_input) = input_writer
            .join()
            .map_err(|_| OcrProviderRefusal::ProviderFailed)?;
        self.graymap = input_buffer;
        let stdout = stdout_reader
            .join()
            .map_err(|_| OcrProviderRefusal::ProviderFailed)??;
        let stderr = stderr_reader
            .join()
            .map_err(|_| OcrProviderRefusal::ProviderFailed)??;
        let BoundedRead {
            bytes: stdout_bytes,
            overflowed: stdout_overflowed,
        } = stdout;
        let BoundedRead {
            bytes: stderr_bytes,
            overflowed: stderr_overflowed,
        } = stderr;
        self.stdout = stdout_bytes;
        self.stderr = stderr_bytes;
        let status = status?;
        wrote_input?;
        if !status.success() || stdout_overflowed || stderr_overflowed {
            return Err(if stdout_overflowed || stderr_overflowed {
                OcrProviderRefusal::OutputCapacity
            } else {
                OcrProviderRefusal::ProviderFailed
            });
        }
        let output = std::str::from_utf8(&self.stdout)
            .map_err(|_| OcrProviderRefusal::InvalidProviderOutput)?;
        visit_tesseract_tsv(output, width, height, visit)
    }
}

struct BoundedRead {
    bytes: Vec<u8>,
    overflowed: bool,
}

fn bounded_read(
    mut reader: impl Read,
    mut bytes: Vec<u8>,
    maximum: usize,
) -> Result<BoundedRead, OcrProviderRefusal> {
    if bytes.capacity() < maximum {
        return Err(OcrProviderRefusal::OutputCapacity);
    }
    bytes.clear();
    let mut scratch = [0u8; 4096];
    let mut overflowed = false;
    loop {
        let count = reader
            .read(&mut scratch)
            .map_err(|_| OcrProviderRefusal::ProviderFailed)?;
        if count == 0 {
            break;
        }
        let remaining = maximum.saturating_sub(bytes.len());
        bytes.extend_from_slice(&scratch[..count.min(remaining)]);
        overflowed |= count > remaining;
    }
    Ok(BoundedRead { bytes, overflowed })
}

fn wait_until(
    child: &mut Child,
    timeout: Duration,
) -> Result<std::process::ExitStatus, OcrProviderRefusal> {
    let started = Instant::now();
    loop {
        if started.elapsed() >= timeout {
            stop(child);
            return Err(OcrProviderRefusal::TimedOut);
        }
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => std::thread::sleep(Duration::from_millis(2)),
            Err(_) => {
                stop(child);
                return Err(OcrProviderRefusal::ProviderFailed);
            }
        }
    }
}

fn stop(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn exact_file(path: &Path) -> Result<PathBuf, OcrProviderRefusal> {
    let path = path
        .canonicalize()
        .map_err(|_| OcrProviderRefusal::InvalidExecutable)?;
    if !fs::metadata(&path)
        .map_err(|_| OcrProviderRefusal::InvalidExecutable)?
        .is_file()
    {
        return Err(OcrProviderRefusal::InvalidExecutable);
    }
    Ok(path)
}

fn digest_file(path: &Path) -> Result<String, OcrProviderRefusal> {
    let bytes = fs::read(path).map_err(|_| OcrProviderRefusal::InvalidExecutable)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

/// Frames an exact grayscale image for a provider accepting portable graymap input.
///
/// The caller owns and preallocates `output`; this function refuses rather than
/// growing it during an admitted Play.
pub fn encode_graymap(
    pixels: &[u8],
    width: u16,
    height: u16,
    output: &mut Vec<u8>,
) -> Result<(), OcrProviderRefusal> {
    let pixel_count = usize::from(width)
        .checked_mul(usize::from(height))
        .filter(|count| *count > 0 && *count == pixels.len())
        .ok_or(OcrProviderRefusal::InvalidImage)?;
    let header = format!("P5\n{width} {height}\n255\n");
    let required = header
        .len()
        .checked_add(pixel_count)
        .ok_or(OcrProviderRefusal::OutputCapacity)?;
    if required > output.capacity() {
        return Err(OcrProviderRefusal::OutputCapacity);
    }
    output.clear();
    output.extend_from_slice(header.as_bytes());
    output.extend_from_slice(pixels);
    Ok(())
}

/// Visits finite word-level candidates from Tesseract's documented TSV shape.
///
/// Non-word rows and negative-confidence rows carry layout metadata rather than
/// observations and are ignored. Any malformed admitted word row fails the
/// whole provider result so partial prose is never presented as exact output.
pub fn visit_tesseract_tsv<'a>(
    tsv: &'a str,
    image_width: u16,
    image_height: u16,
    mut visit: impl FnMut(OcrCandidate<'a>) -> Result<(), OcrProviderRefusal>,
) -> Result<usize, OcrProviderRefusal> {
    let mut count = 0usize;
    for (line_index, line) in tsv.lines().enumerate() {
        if line_index == 0 && line.starts_with("level\t") {
            continue;
        }
        let mut columns = line.splitn(12, '\t');
        let level = parse_u16(columns.next())?;
        for _ in 0..5 {
            columns
                .next()
                .ok_or(OcrProviderRefusal::InvalidProviderOutput)?;
        }
        let left = parse_u16(columns.next())?;
        let top = parse_u16(columns.next())?;
        let width = parse_u16(columns.next())?;
        let height = parse_u16(columns.next())?;
        let confidence = columns
            .next()
            .ok_or(OcrProviderRefusal::InvalidProviderOutput)?;
        let text = columns
            .next()
            .ok_or(OcrProviderRefusal::InvalidProviderOutput)?;
        if level != 5 || confidence.starts_with('-') || text.is_empty() {
            continue;
        }
        if text.len() > MAXIMUM_VISIBLE_TEXT_BYTES || count == MAXIMUM_OCR_ITEMS {
            return Err(OcrProviderRefusal::OutputCapacity);
        }
        let region = ImageRegion {
            x: left,
            y: top,
            width,
            height,
        };
        if width == 0
            || height == 0
            || u32::from(left) + u32::from(width) > u32::from(image_width)
            || u32::from(top) + u32::from(height) > u32::from(image_height)
        {
            return Err(OcrProviderRefusal::InvalidProviderOutput);
        }
        visit(OcrCandidate {
            text,
            region,
            confidence_permille: parse_confidence_permille(confidence)?,
        })?;
        count += 1;
    }
    Ok(count)
}

fn parse_u16(value: Option<&str>) -> Result<u16, OcrProviderRefusal> {
    value
        .ok_or(OcrProviderRefusal::InvalidProviderOutput)?
        .parse()
        .map_err(|_| OcrProviderRefusal::InvalidProviderOutput)
}

fn parse_confidence_permille(value: &str) -> Result<u16, OcrProviderRefusal> {
    let (whole, fractional) = value.split_once('.').unwrap_or((value, ""));
    let whole: u16 = whole
        .parse()
        .map_err(|_| OcrProviderRefusal::InvalidProviderOutput)?;
    if whole > 100 || !fractional.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(OcrProviderRefusal::InvalidProviderOutput);
    }
    let tenths = fractional
        .as_bytes()
        .first()
        .map_or(0, |byte| u16::from(byte - b'0'));
    let confidence = whole
        .checked_mul(10)
        .and_then(|value| value.checked_add(tenths))
        .ok_or(OcrProviderRefusal::InvalidProviderOutput)?;
    if confidence > 1_000 {
        return Err(OcrProviderRefusal::InvalidProviderOutput);
    }
    Ok(confidence)
}

#[cfg(test)]
mod tests;
