use super::*;
use image::{
    codecs::{jpeg::JpegEncoder, png::PngEncoder},
    ExtendedColorType, ImageEncoder,
};

fn candidate() -> ApodImageCandidate {
    decode_apod_candidate(&metadata("2026-11-16", "Credit")).unwrap()
}
fn metadata(date: &str, credit: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({"date":date,"title":"Actual source title","media_type":"image", "hdurl":"https://apod.nasa.gov/apod/test.jpg", "copyright":credit})).unwrap()
}
fn png(width: u32, height: u32, alpha: u8, value: u8) -> Vec<u8> {
    let mut bytes = Vec::new();
    let pixels = [value, 20, 30, alpha].repeat((width * height) as usize);
    PngEncoder::new(&mut bytes)
        .write_image(&pixels, width, height, ExtendedColorType::Rgba8)
        .unwrap();
    bytes
}
fn admission() -> PreparationAdmission {
    PreparationAdmission::admit(
        NaiveDate::from_ymd_opt(2026, 11, 16).unwrap(),
        MAXIMUM_RETAINED_BYTES,
        PREPARATION_BYTES,
    )
    .unwrap()
}
#[test]
fn prepares_opaque_png_and_jpeg_with_deterministic_identity() {
    let bytes = png(20, 10, 255, 10);
    let first = prepare_photo(candidate(), &bytes).unwrap();
    let second = prepare_photo(candidate(), &bytes).unwrap();
    assert_eq!(first.version_sha256(), second.version_sha256());
    assert_eq!(
        first.original_sha256(),
        &<[u8; 32]>::from(Sha256::digest(&bytes))
    );
    assert_eq!((first.width(), first.height()), (20, 10));
    assert!(first.rgba().as_chunks::<4>().0.iter().all(|p| p[3] == 255));
    assert_eq!(first.candidate().credit.as_deref(), Some("Credit"));
    assert_eq!(first.license(), None);
    let mut jpeg = Vec::new();
    JpegEncoder::new(&mut jpeg)
        .encode(&[20; 12], 2, 2, ExtendedColorType::Rgb8)
        .unwrap();
    assert!(prepare_photo(candidate(), &jpeg).is_ok());
}
#[test]
fn rejects_unfit_inputs_before_returning_pixels() {
    assert!(matches!(
        prepare_photo(candidate(), &png(2, 2, 0, 1)),
        Err(PreparationError::TransparentImage)
    ));
    assert!(matches!(
        prepare_photo(candidate(), b"bad"),
        Err(PreparationError::InvalidImage)
    ));
    assert!(matches!(
        prepare_photo(candidate(), b"GIF89a"),
        Err(PreparationError::UnsupportedImage)
    ));
    assert!(matches!(
        prepare_photo(candidate(), &vec![0; MAXIMUM_COMPRESSED_BYTES + 1]),
        Err(PreparationError::ResponseTooLarge)
    ));
    assert!(matches!(
        prepare_photo(candidate(), &png(2049, 1, 255, 1)),
        Err(PreparationError::InvalidImage)
    ));
    let mut source = candidate();
    source.credit = Some("x".repeat(MAXIMUM_CREDIT_BYTES + 1));
    assert!(matches!(
        prepare_photo(source, &png(2, 2, 255, 1)),
        Err(PreparationError::CreditTooLarge)
    ));
}
#[test]
fn returns_exactly_four_distinct_photos_or_bounded_failure() {
    let mut requests = 0;
    let deck = prepare_deck(admission(), |url, bound| {
        requests += 1;
        if bound == MAXIMUM_APOD_RESPONSE_BYTES {
            Ok(metadata(url.rsplit('=').next().unwrap(), "Credit"))
        } else {
            Ok(png(2, 2, 255, requests as u8))
        }
    })
    .unwrap();
    assert_eq!(requests, 8);
    assert_eq!(deck.photos().len(), 4);
    assert!(deck.retained_bytes() <= MAXIMUM_RETAINED_BYTES);
    let mut failed = 0;
    let result = prepare_deck(admission(), |_, _| {
        failed += 1;
        Err(PreparationError::Transport)
    });
    assert!(matches!(
        result,
        Err(PreparationError::IncompleteDeck {
            prepared: 0,
            attempts: MAXIMUM_ATTEMPTS
        })
    ));
    assert_eq!(failed, MAXIMUM_ATTEMPTS);
}
#[test]
fn repeats_and_wrong_dates_never_complete_a_deck() {
    let result = prepare_deck(admission(), |url, bound| {
        if bound == MAXIMUM_APOD_RESPONSE_BYTES {
            Ok(metadata(url.rsplit('=').next().unwrap(), "Credit"))
        } else {
            Ok(png(2, 2, 255, 1))
        }
    });
    assert!(matches!(
        result,
        Err(PreparationError::IncompleteDeck { prepared: 1, .. })
    ));
    assert!(
        PreparationAdmission::admit(NaiveDate::from_ymd_opt(2026, 11, 16).unwrap(), 0, 0).is_err()
    );
}

#[test]
fn rate_limited_provider_stops_deck_preparation_without_another_request() {
    let mut requests = 0;
    let result = prepare_deck(admission(), |_, _| {
        requests += 1;
        Err(PreparationError::ProviderRateLimited)
    });
    assert!(matches!(result, Err(PreparationError::ProviderRateLimited)));
    assert_eq!(requests, 1);
}
