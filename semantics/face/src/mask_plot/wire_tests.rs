use super::*;
use alloc::format;

fn encoded(bytes: &[u8]) -> String {
    compress_to_vec_zlib(bytes, 6)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn preflight_expands_and_counts_without_deserializing_a_plan() {
    let bytes = br#"{"nodes":[{"label":"original"}],"numbers":[1,2]}"#;
    let (original, shape) =
        PlannedMaskPlot::preflight_json_plan(ENCODING, bytes.len(), &encoded(bytes)).unwrap();
    assert_eq!(original, bytes);
    assert_eq!(shape.objects, 2);
    assert_eq!(shape.object_fields, 3);
    assert_eq!(shape.object_or_string_array_elements, 1);
    assert_eq!(shape.numeric_array_elements, 2);
    // This deliberately is not an ordinary Plan. Model admission is separate.
    assert!(serde_json::from_slice::<Plan>(&original).is_err());
}

#[test]
fn preflight_preserves_wire_bounds_and_exact_stream_consumption() {
    let bytes = b"{}";
    let hex = encoded(bytes);
    assert!(PlannedMaskPlot::preflight_json_plan("different", bytes.len(), &hex).is_err());
    assert!(PlannedMaskPlot::preflight_json_plan(ENCODING, MAX_EXPANDED_BYTES + 1, &hex).is_err());
    assert!(PlannedMaskPlot::preflight_json_plan(ENCODING, 0, &hex).is_err());
    assert!(PlannedMaskPlot::preflight_json_plan(ENCODING, 1, &hex).is_err());
    assert!(PlannedMaskPlot::preflight_json_plan(ENCODING, 3, &hex).is_err());
    assert!(PlannedMaskPlot::preflight_json_plan(ENCODING, 2, &format!("{hex}00")).is_err());
    assert!(PlannedMaskPlot::preflight_json_plan(ENCODING, 2, &hex[..hex.len() - 2]).is_err());
    assert!(PlannedMaskPlot::preflight_json_plan(ENCODING, 2, "GG").is_err());
    let malformed = b"[1,]";
    assert_eq!(
        PlannedMaskPlot::preflight_json_plan(ENCODING, malformed.len(), &encoded(malformed)),
        Err("Mask Plan JSON preflight")
    );
}
