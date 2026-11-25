//! NASA's bounded APOD Basic list and direct image preparation.
use super::{
    prepare_photo, PreparationAdmission, PreparationError, PreparedDeck, DECK_LEN,
    MAXIMUM_APOD_RESPONSE_BYTES, MAXIMUM_ATTEMPTS, MAXIMUM_COMPRESSED_BYTES, MAXIMUM_CREDIT_BYTES,
};
use crate::ambient_wallpaper::{decode_response, Response};

const LIST: &str = "https://science.nasa.gov/wp-json/wp/v2/apod-basic";
const ASSET_PREFIX: &str = "https://assets.science.nasa.gov/dynamicimage/assets/science/";

pub(super) fn prepare_deck(
    admission: PreparationAdmission,
    mut fetch: impl FnMut(&str, usize) -> Result<Vec<u8>, PreparationError>,
) -> Result<PreparedDeck, PreparationError> {
    let url =
        format!("{LIST}?per_page={MAXIMUM_ATTEMPTS}&_fields=date,title,media_type,hdurl,copyright");
    let metadata = fetch(&url, MAXIMUM_APOD_RESPONSE_BYTES)?;
    if metadata.len() > MAXIMUM_APOD_RESPONSE_BYTES {
        return Err(PreparationError::InvalidMetadata);
    }
    let mut entries: Vec<Response<'_>> =
        serde_json::from_slice(&metadata).map_err(|_| PreparationError::InvalidMetadata)?;
    if entries.len() > MAXIMUM_ATTEMPTS {
        return Err(PreparationError::InvalidMetadata);
    }
    entries.sort_by(|left, right| right.date.cmp(&left.date));
    let latest = admission.latest.to_string();
    let mut photos = Vec::with_capacity(DECK_LEN);
    let mut attempts = 0;
    for mut entry in entries {
        if entry.date.as_ref() > latest.as_str() {
            continue;
        }
        attempts += 1;
        let Some((path, _)) = entry.hdurl.as_deref().and_then(|url| url.split_once('?')) else {
            continue;
        };
        if !path.starts_with(ASSET_PREFIX) {
            continue;
        }
        entry.hdurl = Some(path.to_owned().into());
        let Ok(mut candidate) = decode_response(entry) else {
            continue;
        };
        if candidate
            .credit
            .as_ref()
            .is_some_and(|credit| credit.len() > MAXIMUM_CREDIT_BYTES)
        {
            continue;
        }
        // Use only our own fixed transformation on NASA's exact image path.
        candidate.asset_url.push_str("?w=1024&h=1024&fit=clip");
        let photo = fetch(&candidate.asset_url, MAXIMUM_COMPRESSED_BYTES)
            .and_then(|bytes| prepare_photo(candidate, &bytes));
        match photo {
            Err(PreparationError::ProviderRateLimited) => {
                return Err(PreparationError::ProviderRateLimited);
            }
            Ok(photo)
                if !photos.iter().any(|previous: &super::PreparedPhoto| {
                    previous.original_sha256() == photo.original_sha256()
                }) =>
            {
                photos.push(photo)
            }
            _ => {}
        }
        if photos.len() == DECK_LEN {
            return Ok(PreparedDeck {
                photos: photos.try_into().unwrap(),
            });
        }
    }
    Err(PreparationError::IncompleteDeck {
        prepared: photos.len(),
        attempts,
    })
}
