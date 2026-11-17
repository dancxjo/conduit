//! Explicit operator-reviewed alternative to quota-bearing APOD API discovery.
//! A pinned manifest supplies exact reviewed metadata and published NASA image
//! renditions. It grants no authority, infers no license, and never falls back.
use super::{
    acquisition::{self, PreparationAdmission, PreparationError, PreparedDeck},
    decode_apod_candidate, MAXIMUM_APOD_RESPONSE_BYTES,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    profile: String,
    photos: [Photo; 4],
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Photo {
    candidate: Candidate,
    sha256: [u8; 32],
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Candidate {
    date: String,
    title: String,
    media_type: String,
    hdurl: String,
    copyright: Option<String>,
}

/// Same full preparation admission and decoder as APOD API acquisition. Exactly
/// four asset fetches at most, no metadata requests, redirects, retries or extra
/// discovery. The operator independently reviews primary-source provenance and
/// pins both this manifest and each original compressed image's checksum.
pub fn prepare_deck(
    admission: PreparationAdmission,
    body: &[u8],
    expected_sha256: [u8; 32],
    mut fetch: impl FnMut(&str, usize) -> Result<Vec<u8>, PreparationError>,
) -> Result<PreparedDeck, PreparationError> {
    if body.len() > MAXIMUM_APOD_RESPONSE_BYTES {
        return Err(PreparationError::ResponseTooLarge);
    }
    if <[u8; 32]>::from(Sha256::digest(body)) != expected_sha256 {
        return Err(PreparationError::InvalidMetadata);
    }
    let manifest: Manifest =
        serde_json::from_slice(body).map_err(|_| PreparationError::InvalidMetadata)?;
    if manifest.profile != "reviewed-apod-candidates-v1" {
        return Err(PreparationError::InvalidMetadata);
    }
    // Validate all metadata before the first acquisition effect.
    let mut candidates = Vec::with_capacity(4);
    for p in manifest.photos {
        let encoded =
            serde_json::to_vec(&p.candidate).map_err(|_| PreparationError::InvalidMetadata)?;
        let candidate =
            decode_apod_candidate(&encoded).map_err(|_| PreparationError::InvalidMetadata)?;
        if candidate.date > admission.latest_date().to_string()
            || candidate
                .credit
                .as_ref()
                .is_some_and(|c| c.len() > acquisition::MAXIMUM_CREDIT_BYTES)
            || candidates
                .iter()
                .any(|(prior, _): &(super::ApodImageCandidate, [u8; 32])| {
                    prior.date == candidate.date
                })
        {
            return Err(PreparationError::InvalidMetadata);
        }
        candidates.push((candidate, p.sha256));
    }
    let mut photos = Vec::with_capacity(4);
    for (candidate, expected) in candidates {
        let bytes = fetch(&candidate.asset_url, acquisition::MAXIMUM_COMPRESSED_BYTES)?;
        if bytes.len() > acquisition::MAXIMUM_COMPRESSED_BYTES {
            return Err(PreparationError::ResponseTooLarge);
        }
        if <[u8; 32]>::from(Sha256::digest(&bytes)) != expected {
            return Err(PreparationError::InvalidImage);
        }
        let photo = acquisition::prepare_photo(candidate, &bytes)?;
        if photos.iter().any(|prior: &acquisition::PreparedPhoto| {
            prior.original_sha256() == photo.original_sha256()
        }) {
            return Err(PreparationError::InvalidImage);
        }
        photos.push(photo);
    }
    Ok(PreparedDeck::from_checked_cache(
        photos
            .try_into()
            .map_err(|_| PreparationError::InvalidImage)?,
    ))
}
pub fn prepare_deck_https(
    admission: PreparationAdmission,
    body: &[u8],
    expected_sha256: [u8; 32],
) -> Result<PreparedDeck, PreparationError> {
    prepare_deck(
        admission,
        body,
        expected_sha256,
        acquisition::bounded_https_transport(),
    )
}

#[cfg(test)]
#[path = "candidate_manifest/tests.rs"]
mod tests;
