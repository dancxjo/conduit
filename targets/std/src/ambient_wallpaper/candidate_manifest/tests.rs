use super::*;
use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder};
fn fixture() -> (Vec<u8>, [u8; 32], Vec<Vec<u8>>) {
    let bytes: Vec<_> = (1..=4)
        .map(|n| {
            let mut bytes = Vec::new();
            PngEncoder::new(&mut bytes)
                .write_image(&[n, 2, 3, 255], 1, 1, ExtendedColorType::Rgba8)
                .unwrap();
            bytes
        })
        .collect();
    let photos:Vec<_>=bytes.iter().enumerate().map(|(i,b)|serde_json::json!({
        "candidate":{"date":format!("2024-01-0{}",i+1),"title":"Reviewed actual metadata","media_type":"image","hdurl":"https://assets.science.nasa.gov/apod/actual.jpeg","copyright":"Exact credit"},
        "sha256":<[u8;32]>::from(Sha256::digest(b))
    })).collect();
    let body = serde_json::to_vec(
        &serde_json::json!({"profile":"reviewed-apod-candidates-v1","photos":photos}),
    )
    .unwrap();
    let checksum = Sha256::digest(&body).into();
    (body, checksum, bytes)
}
fn admission() -> PreparationAdmission {
    PreparationAdmission::admit(
        chrono::NaiveDate::from_ymd_opt(2024, 1, 5).unwrap(),
        acquisition::MAXIMUM_RETAINED_BYTES,
        acquisition::PREPARATION_BYTES,
    )
    .unwrap()
}
#[test]
fn four_explicit_pinned_assets_share_existing_real_image_preparation() {
    let (body, checksum, bytes) = fixture();
    let mut index = 0;
    let deck = prepare_deck(admission(), &body, checksum, |url, bound| {
        assert_eq!(url, "https://assets.science.nasa.gov/apod/actual.jpeg");
        assert_eq!(bound, acquisition::MAXIMUM_COMPRESSED_BYTES);
        let b = bytes[index].clone();
        index += 1;
        Ok(b)
    })
    .unwrap();
    assert_eq!(index, 4);
    assert_eq!(deck.photos().len(), 4);
    assert_eq!(
        deck.photos()[0].candidate().credit.as_deref(),
        Some("Exact credit")
    );
    assert_eq!(deck.photos()[0].license(), None);
}
#[test]
fn checksum_and_all_metadata_are_checked_before_transport_and_assets_are_pinned() {
    let (body, checksum, _) = fixture();
    assert!(prepare_deck(admission(), &body, [0; 32], |_, _| panic!("no request")).is_err());
    let mut value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    value["photos"][3]["candidate"]["hdurl"] = serde_json::json!("https://evil.test/image.jpg");
    let invalid = serde_json::to_vec(&value).unwrap();
    assert!(prepare_deck(
        admission(),
        &invalid,
        Sha256::digest(&invalid).into(),
        |_, _| panic!("no request")
    )
    .is_err());
    assert!(prepare_deck(admission(), &body, checksum, |_, _| Ok(vec![1, 2, 3])).is_err());
}
