use super::*;

fn response(asset: Option<&str>, media: &str, date: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "date": date,
        "title": "A test nebula",
        "media_type": media,
        "url": "https://science.nasa.gov/image-article/a-test-nebula/",
        "hdurl": asset,
        "copyright": "<b>Credit:</b> Example photographer"
    }))
    .unwrap()
}

#[test]
fn current_api_uses_asset_not_article_and_preserves_credit() {
    let asset = "https://assets.science.nasa.gov/apod/nebula.jpg";
    let candidate = decode_apod_candidate(&response(Some(asset), "image", "2026-11-16")).unwrap();
    assert_eq!(candidate.asset_url, asset);
    assert_eq!(
        candidate.credit.as_deref(),
        Some("<b>Credit:</b> Example photographer")
    );
    assert_eq!(
        decode_apod_candidate(&response(None, "image", "2026-11-16")),
        Err(ApodRefusal::MissingAsset)
    );
}

#[test]
fn refuses_non_image_and_ambiguous_or_unoffered_targets() {
    let asset = "https://apod.nasa.gov/apod/test.jpg";
    assert_eq!(
        decode_apod_candidate(&response(Some(asset), "video", "2026-11-16")),
        Err(ApodRefusal::UnsupportedMedia)
    );
    for asset in [
        "http://apod.nasa.gov/test.jpg",
        "https://apod.nasa.gov.evil.test/test.jpg",
        "https://apod.nasa.gov@evil.test/test.jpg",
        "https://apod.nasa.gov/test.html",
        "https://apod.nasa.gov/../test.jpg",
        "https://apod.nasa.gov/%2e%2e/test.jpg",
        "https://apod.nasa.gov/test.jpg?redirect=x",
        "https://apod.nasa.gov/test\\x.jpg",
    ] {
        assert_eq!(
            decode_apod_candidate(&response(Some(asset), "image", "2026-11-16")),
            Err(ApodRefusal::UnsupportedAsset),
            "{asset}"
        );
    }
}

#[test]
fn refuses_invalid_dates_and_oversized_or_malformed_metadata() {
    let asset = Some("https://apod.nasa.gov/test.png");
    for date in ["2026-02-29", "1995-06-15", "2026-1-01"] {
        assert_eq!(
            decode_apod_candidate(&response(asset, "image", date)),
            Err(ApodRefusal::InvalidDate)
        );
    }
    assert_eq!(
        decode_apod_candidate(&vec![b' '; MAXIMUM_APOD_RESPONSE_BYTES + 1]),
        Err(ApodRefusal::ResponseTooLarge)
    );
    assert_eq!(
        decode_apod_candidate(b"<html>unavailable</html>"),
        Err(ApodRefusal::MalformedResponse)
    );
}
