use conduit_ai::CompatibleMetrics;
use conduit_plot::rust_binding::NativeRustBinding;

fn assert_native_round_trip(value: CompatibleMetrics) {
    let structured = value.into_structured().unwrap();
    assert_eq!(
        CompatibleMetrics::from_structured(structured).unwrap(),
        value
    );
}

#[test]
fn compatible_metrics_exhaust_every_finite_boolean_value() {
    for bits in 0_u8..8 {
        let value = CompatibleMetrics::new(bits & 1 != 0, bits & 2 != 0, bits & 4 != 0).unwrap();
        assert_native_round_trip(value);
    }
}

#[test]
fn compatible_metrics_preserve_the_established_serde_shapes() {
    let value = CompatibleMetrics::new(true, false, true).unwrap();
    assert_eq!(
        serde_json::to_string(&value).unwrap(),
        r#"{"cosine_similarity":true,"dot_product_similarity":false,"squared_euclidean_distance":true}"#
    );
    assert_eq!(postcard::to_allocvec(&value).unwrap(), [1, 0, 1]);
    assert_eq!(
        serde_json::from_str::<CompatibleMetrics>(
            r#"{"cosine_similarity":true,"dot_product_similarity":false,"squared_euclidean_distance":true}"#,
        )
        .unwrap(),
        value
    );
    assert_eq!(
        postcard::from_bytes::<CompatibleMetrics>(&[1, 0, 1]).unwrap(),
        value
    );
}
