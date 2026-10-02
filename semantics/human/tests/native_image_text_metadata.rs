use conduit_human::ImageTextMetadata;
use conduit_plot::rust_binding::NativeRustBinding;

#[test]
fn image_text_metadata_round_trips_exact_text_boundaries() {
    let metadata = ImageTextMetadata::new("k".repeat(64), "v".repeat(256)).unwrap();
    assert_eq!(metadata.key().len(), 64);
    assert_eq!(metadata.value().len(), 256);
    let structured = metadata.clone().into_structured().unwrap();
    assert_eq!(
        ImageTextMetadata::from_structured(structured).unwrap(),
        metadata
    );

    assert!(ImageTextMetadata::new(String::new(), String::new()).is_err());
    assert!(ImageTextMetadata::new("k".repeat(65), String::new()).is_err());
    assert!(ImageTextMetadata::new("key".into(), "v".repeat(257)).is_err());
}
