//! Bounded interpretation of an acquired NASA APOD response.
//!
//! This decoder neither fetches nor grants network/resource authority. Its asset
//! URL is a proposed acquisition input, never an ImageResource or trusted pixel
//! buffer. The caller must admit acquisition, decode and original image custody.
//! Protocol: <https://github.com/nasa/apod-api/blob/master/README.md>.

pub mod acquisition;
#[cfg(unix)]
pub mod cache;
#[cfg(unix)]
pub mod candidate_manifest;
pub mod selection_wire;

use serde::Deserialize;
use std::borrow::Cow;

pub const MAXIMUM_APOD_RESPONSE_BYTES: usize = 64 * 1024;
const MAXIMUM_ASSET_URL_BYTES: usize = 2048;

#[derive(Debug, PartialEq, Eq)]
pub enum ApodRefusal {
    ResponseTooLarge,
    MalformedResponse,
    UnsupportedMedia,
    InvalidDate,
    MissingAsset,
    UnsupportedAsset,
    InvalidTitle,
    InvalidCredit,
}

/// Metadata only. In particular, no license is inferred from NASA hosting.
#[derive(Debug, PartialEq, Eq)]
pub struct ApodImageCandidate {
    pub date: String,
    pub title: String,
    pub asset_url: String,
    /// Provider-supplied credit, retained verbatim; may contain HTML.
    /// Consumers must render this as text or apply a separately reviewed decoder.
    pub credit: Option<String>,
}

#[derive(Deserialize)]
struct Response<'a> {
    #[serde(borrow)]
    date: Cow<'a, str>,
    #[serde(borrow)]
    title: Cow<'a, str>,
    #[serde(borrow)]
    media_type: Cow<'a, str>,
    #[serde(borrow)]
    hdurl: Option<Cow<'a, str>>,
    #[serde(borrow)]
    copyright: Option<Cow<'a, str>>,
}

/// Accept only NASA's current direct-asset field. `url` is deliberately ignored:
/// the current APOD API uses it for the article, not the image bytes. Restricting
/// the candidate host is transport policy, not possession of network authority.
pub fn decode_apod_candidate(body: &[u8]) -> Result<ApodImageCandidate, ApodRefusal> {
    if body.len() > MAXIMUM_APOD_RESPONSE_BYTES {
        return Err(ApodRefusal::ResponseTooLarge);
    }
    let response: Response<'_> =
        serde_json::from_slice(body).map_err(|_| ApodRefusal::MalformedResponse)?;
    if response.media_type != "image" {
        return Err(ApodRefusal::UnsupportedMedia);
    }
    // A real civil date, not merely ten plausible bytes. Parsing also rejects
    // dates before APOD began and future dates are left to request correlation.
    let date = chrono::NaiveDate::parse_from_str(&response.date, "%Y-%m-%d")
        .map_err(|_| ApodRefusal::InvalidDate)?;
    if date.to_string() != response.date || response.date.as_ref() < "1995-06-16" {
        return Err(ApodRefusal::InvalidDate);
    }
    if response.title.trim().is_empty()
        || response.title.len() > 160
        || response.title.chars().any(char::is_control)
    {
        return Err(ApodRefusal::InvalidTitle);
    }
    let asset = response.hdurl.ok_or(ApodRefusal::MissingAsset)?;
    if asset.len() > MAXIMUM_ASSET_URL_BYTES
        || asset.chars().any(|c| c.is_control() || c.is_whitespace())
    {
        return Err(ApodRefusal::UnsupportedAsset);
    }
    let path = asset
        .strip_prefix("https://assets.science.nasa.gov/")
        .or_else(|| asset.strip_prefix("https://apod.nasa.gov/"))
        .ok_or(ApodRefusal::UnsupportedAsset)?;
    // Refuse alternate URL parsing interpretations and non-image article paths.
    if path.is_empty()
        || path.contains(['\\', '#', '?', '%'])
        || path.split('/').any(|part| matches!(part, "." | ".."))
        || ![".jpg", ".jpeg", ".png"].iter().any(|ext| {
            path.get(path.len().saturating_sub(ext.len())..)
                .is_some_and(|suffix| suffix.eq_ignore_ascii_case(ext))
        })
    {
        return Err(ApodRefusal::UnsupportedAsset);
    }
    if response.copyright.as_ref().is_some_and(|credit| {
        credit.trim().is_empty() || credit.len() > 4096 || credit.contains('\0')
    }) {
        return Err(ApodRefusal::InvalidCredit);
    }
    Ok(ApodImageCandidate {
        date: response.date.into_owned(),
        title: response.title.into_owned(),
        asset_url: asset.into_owned(),
        credit: response.copyright.map(Cow::into_owned),
    })
}

#[cfg(test)]
#[path = "ambient_wallpaper/tests.rs"]
mod tests;
