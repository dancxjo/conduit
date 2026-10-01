use conduit_ai::SourceExtractionProfile;
use conduit_form::rust_binding::NativeRustBinding;

fn assert_native_round_trip(value: SourceExtractionProfile) {
    let structured = value.clone().into_structured().unwrap();
    assert_eq!(
        SourceExtractionProfile::from_structured(structured).unwrap(),
        value
    );
}

#[test]
fn source_extraction_profiles_round_trip_through_the_exact_native_payload_type() {
    assert_native_round_trip(SourceExtractionProfile::text_utf8(2).unwrap());
    assert_native_round_trip(SourceExtractionProfile::structured_items(3).unwrap());
    assert_native_round_trip(SourceExtractionProfile::resource_metadata(5).unwrap());
}

#[test]
fn source_extraction_profiles_preserve_serde_and_postcard_shapes() {
    for (value, json, bytes) in [
        (
            SourceExtractionProfile::text_utf8(2).unwrap(),
            r#"{"TextUtf8":{"overlap_bytes":2}}"#,
            vec![0, 2],
        ),
        (
            SourceExtractionProfile::structured_items(3).unwrap(),
            r#"{"StructuredItems":{"overlap_items":3}}"#,
            vec![1, 3],
        ),
        (
            SourceExtractionProfile::resource_metadata(5).unwrap(),
            r#"{"ResourceMetadata":{"overlap_items":5}}"#,
            vec![2, 5],
        ),
    ] {
        assert_eq!(serde_json::to_string(&value).unwrap(), json);
        assert_eq!(postcard::to_allocvec(&value).unwrap(), bytes);
        assert_eq!(
            serde_json::from_str::<SourceExtractionProfile>(json).unwrap(),
            value
        );
        assert_eq!(
            postcard::from_bytes::<SourceExtractionProfile>(&bytes).unwrap(),
            value
        );
    }
}
