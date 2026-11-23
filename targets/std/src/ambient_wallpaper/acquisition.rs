//! Explicit, finite preparation; never call from a Mask, Step, or runtime read.
//! Returned bytes are prepared photographs, not installed or authorized resources.
use super::{decode_apod_candidate, ApodImageCandidate, MAXIMUM_APOD_RESPONSE_BYTES};
use chrono::NaiveDate;
use image::{ImageDecoder, ImageFormat, ImageReader};
use sha2::{Digest, Sha256};
use std::{io::Cursor, time::Duration};

pub const DECK_LEN: usize = 4;
pub const MAXIMUM_ATTEMPTS: usize = 12;
pub const MAXIMUM_COMPRESSED_BYTES: usize = 4 * 1024 * 1024;
pub const MAXIMUM_CREDIT_BYTES: usize = 256;
pub const MAXIMUM_RETAINED_BYTES: usize = DECK_LEN * (1024 * 600 * 4 + 4608);
/// Conservative preparation reservation: decoder budget, decoded image, RGBA,
/// resize buffers, compressed input, metadata and the complete retained deck.
/// Worst pixel accounting: 16MiB decoded + 16MiB RGBA conversion + 4MiB
/// compressed + 9.375MiB retained deck + 2.344MiB resized output <48MiB.
/// These maxima overlap conservatively; processing is strictly sequential.
/// Decoder max_alloc is best effort in image; strict dimensions additionally
/// bound pixel storage. This reservation excludes TLS/allocator/OS overhead.
pub const PREPARATION_BYTES: usize = 48 * 1024 * 1024;

/// Constructing this value records the caller's explicit preparation admission.
/// It does not grant network authority; the supplied transport owns that grant.
pub struct PreparationAdmission {
    latest: NaiveDate,
}
impl PreparationAdmission {
    pub fn admit(
        latest: NaiveDate,
        retained_bytes: usize,
        preparation_bytes: usize,
    ) -> Result<Self, PreparationError> {
        if latest.to_string().as_str() < "1995-06-16"
            || retained_bytes < MAXIMUM_RETAINED_BYTES
            || preparation_bytes < PREPARATION_BYTES
        {
            return Err(PreparationError::InsufficientAdmission);
        }
        Ok(Self { latest })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum PreparationError {
    InsufficientAdmission,
    ClockUnavailable,
    Transport,
    ResponseTooLarge,
    InvalidMetadata,
    CreditTooLarge,
    InvalidImage,
    UnsupportedImage,
    TransparentImage,
    IncompleteDeck { prepared: usize, attempts: usize },
}

pub struct PreparedPhoto {
    candidate: ApodImageCandidate,
    rgba: Box<[u8]>,
    width: u32,
    height: u32,
    original_sha256: [u8; 32],
    version_sha256: [u8; 32],
}
impl PreparedPhoto {
    pub fn candidate(&self) -> &ApodImageCandidate {
        &self.candidate
    }
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn original_sha256(&self) -> &[u8; 32] {
        &self.original_sha256
    }
    pub fn version_sha256(&self) -> &[u8; 32] {
        &self.version_sha256
    }
    /// Absence of an explicit license remains unknown, even for NASA hosting.
    pub fn license(&self) -> Option<&str> {
        None
    }
    pub fn source_page(&self) -> String {
        format!(
            "https://apod.nasa.gov/apod/ap{}.html",
            self.candidate.date.replace('-', "").get(2..).unwrap_or("")
        )
    }
    pub fn retained_bytes(&self) -> usize {
        self.rgba.len()
            + self.candidate.date.capacity()
            + self.candidate.title.capacity()
            + self.candidate.asset_url.capacity()
            + self.candidate.credit.as_ref().map_or(0, String::capacity)
    }
}

pub struct PreparedDeck {
    photos: [PreparedPhoto; DECK_LEN],
}
impl PreparedDeck {
    pub fn photos(&self) -> &[PreparedPhoto; DECK_LEN] {
        &self.photos
    }
    pub fn retained_bytes(&self) -> usize {
        self.photos.iter().map(PreparedPhoto::retained_bytes).sum()
    }
}

/// Transport must honor the supplied body limit *while reading*, not after
/// buffering, and perform one request with no hidden retry or redirect. At most
/// 12 metadata and 12 asset calls occur. Dates are fixed before each request;
/// failed days consume attempts. No placeholder or partial deck is returned.
pub fn prepare_deck(
    admission: PreparationAdmission,
    mut fetch: impl FnMut(&str, usize) -> Result<Vec<u8>, PreparationError>,
) -> Result<PreparedDeck, PreparationError> {
    let mut photos = Vec::with_capacity(DECK_LEN);
    let mut date = admission.latest;
    let mut attempts = 0;
    for _ in 0..MAXIMUM_ATTEMPTS {
        if date.to_string().as_str() < "1995-06-16" {
            break;
        }
        attempts += 1;
        let url = format!("https://api.nasa.gov/planetary/apod?api_key=DEMO_KEY&date={date}");
        let photo = (|| {
            let body = fetch(&url, MAXIMUM_APOD_RESPONSE_BYTES)?;
            let candidate =
                decode_apod_candidate(&body).map_err(|_| PreparationError::InvalidMetadata)?;
            if candidate.date != date.to_string() {
                return Err(PreparationError::InvalidMetadata);
            }
            if candidate
                .credit
                .as_ref()
                .is_some_and(|credit| credit.len() > MAXIMUM_CREDIT_BYTES)
            {
                return Err(PreparationError::CreditTooLarge);
            }
            let bytes = fetch(&candidate.asset_url, MAXIMUM_COMPRESSED_BYTES)?;
            prepare_photo(candidate, &bytes)
        })();
        if let Ok(photo) = photo {
            if !photos
                .iter()
                .any(|p: &PreparedPhoto| p.original_sha256 == photo.original_sha256)
            {
                photos.push(photo);
            }
            if photos.len() == DECK_LEN {
                return Ok(PreparedDeck {
                    photos: photos.try_into().ok().unwrap(),
                });
            }
        }
        let Some(previous) = date.pred_opt() else {
            break;
        };
        date = previous;
    }
    Err(PreparationError::IncompleteDeck {
        prepared: photos.len(),
        attempts,
    })
}

pub(super) fn prepare_photo(
    candidate: ApodImageCandidate,
    bytes: &[u8],
) -> Result<PreparedPhoto, PreparationError> {
    if bytes.len() > MAXIMUM_COMPRESSED_BYTES {
        return Err(PreparationError::ResponseTooLarge);
    }
    if candidate
        .credit
        .as_ref()
        .is_some_and(|c| c.len() > MAXIMUM_CREDIT_BYTES)
    {
        return Err(PreparationError::CreditTooLarge);
    }
    let format = image::guess_format(bytes).map_err(|_| PreparationError::InvalidImage)?;
    if !matches!(format, ImageFormat::Jpeg | ImageFormat::Png) {
        return Err(PreparationError::UnsupportedImage);
    }
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(2048);
    limits.max_image_height = Some(2048);
    limits.max_alloc = Some(16 * 1024 * 1024);
    reader.limits(limits);
    let decoder = reader
        .into_decoder()
        .map_err(|_| PreparationError::InvalidImage)?;
    let (width, height) = decoder.dimensions();
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > 4_194_304 {
        return Err(PreparationError::InvalidImage);
    }
    let decoded =
        image::DynamicImage::from_decoder(decoder).map_err(|_| PreparationError::InvalidImage)?;
    let rgba = decoded.into_rgba8();
    if rgba.pixels().any(|pixel| pixel[3] != 255) {
        return Err(PreparationError::TransparentImage);
    }
    let ratio = (1024.0 / f64::from(width))
        .min(600.0 / f64::from(height))
        .min(1.0);
    let out_width = (f64::from(width) * ratio).floor().max(1.0) as u32;
    let out_height = (f64::from(height) * ratio).floor().max(1.0) as u32;
    let scaled = image::imageops::resize(
        &rgba,
        out_width,
        out_height,
        image::imageops::FilterType::Nearest,
    );
    let (width, height) = scaled.dimensions();
    let rgba = scaled.into_raw().into_boxed_slice();
    let original_sha256: [u8; 32] = Sha256::digest(bytes).into();
    let mut version = Sha256::new();
    version.update(b"conduit-ambient-rgba-v1\0");
    version.update(original_sha256);
    version.update(width.to_le_bytes());
    version.update(height.to_le_bytes());
    version.update(&rgba);
    let photo = PreparedPhoto {
        candidate,
        rgba,
        width,
        height,
        original_sha256,
        version_sha256: version.finalize().into(),
    };
    if photo.retained_bytes() > photo.rgba.len() + 4608 {
        return Err(PreparationError::InsufficientAdmission);
    }
    Ok(photo)
}

/// Real HTTPS transport: fixed allowlist inherited from candidate validation,
/// no redirects, no application retries, 20-second complete-request deadline.
/// Error details deliberately omit request URLs and provider response bodies.
pub fn prepare_deck_https(
    admission: PreparationAdmission,
) -> Result<PreparedDeck, PreparationError> {
    prepare_deck(admission, bounded_https_transport())
}
pub(super) fn bounded_https_transport(
) -> impl FnMut(&str, usize) -> Result<Vec<u8>, PreparationError> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .max_redirects(0)
        .timeout_global(Some(Duration::from_secs(20)))
        .build()
        .into();
    move |url, limit| {
        let mut response = agent
            .get(url)
            .call()
            .map_err(|_| PreparationError::Transport)?;
        if response.status() != 200 {
            return Err(PreparationError::Transport);
        }
        response
            .body_mut()
            .with_config()
            .limit(limit as u64)
            .read_to_vec()
            .map_err(|_| PreparationError::Transport)
    }
}

#[cfg(test)]
#[path = "acquisition/tests.rs"]
mod tests;

/// Prepare the current public photo deck under the fixed acquisition budget.
pub fn prepare_current_public_deck() -> Result<PreparedDeck, PreparationError> {
    let elapsed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| PreparationError::ClockUnavailable)?;
    let seconds =
        i64::try_from(elapsed.as_secs()).map_err(|_| PreparationError::ClockUnavailable)?;
    let date = chrono::DateTime::from_timestamp(seconds, elapsed.subsec_nanos())
        .ok_or(PreparationError::ClockUnavailable)?
        .date_naive();
    let admission = PreparationAdmission::admit(date, MAXIMUM_RETAINED_BYTES, PREPARATION_BYTES)?;
    prepare_deck_https(admission)
}
