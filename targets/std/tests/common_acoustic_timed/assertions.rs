//! Independent state and exact sample checks for timed parent observations.
pub fn check_states(states: &[Vec<u8>]) {
    use conduit_core::{StructuredInfoValue, StructuredInfoValueShape};
    fn field(value: &StructuredInfoValue, name: &str) -> StructuredInfoValue {
        let StructuredInfoValueShape::Record(fields) = value.shape() else {
            panic!("record")
        };
        fields
            .iter()
            .find(|field| field.name() == name)
            .unwrap()
            .value()
            .clone()
    }
    let expected = [
        "sample",
        "stale_epoch",
        "late_control",
        "future_control",
        "sample",
        "sample",
        "sample",
    ];
    assert_eq!(states.len(), expected.len());
    let mut first = None;
    for (encoded, expected) in states.iter().zip(expected) {
        let value = StructuredInfoValue::from_canonical_bytes(encoded).unwrap();
        let StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
            panic!("stage")
        };
        assert_eq!(tag, "context");
        let cursor = field(payload, "cursor");
        let status = field(&cursor, "status");
        let StructuredInfoValueShape::Variant { tag, .. } = status.shape() else {
            panic!("status")
        };
        assert_eq!(tag, expected);
        if first.is_none() {
            first = Some(cursor.clone());
        }
        if expected != "sample" {
            for name in [
                "oscillator",
                "resonator",
                "sample",
                "clipped",
                "history_count",
                "history_a",
                "history_b",
                "history_c",
                "history_d",
            ] {
                assert_eq!(field(&cursor, name), field(first.as_ref().unwrap(), name));
            }
        }
    }
    let value = StructuredInfoValue::from_canonical_bytes(states.last().unwrap()).unwrap();
    let StructuredInfoValueShape::Variant { payload, .. } = value.shape() else {
        panic!("stage")
    };
    let cursor = field(payload, "cursor");
    for (name, expected) in ["history_a", "history_b", "history_c", "history_d"]
        .into_iter()
        .zip([24_i128, 104, 263, 425])
    {
        let value = field(&cursor, name);
        let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
            panic!("sample")
        };
        assert_eq!(i128::from_le_bytes(bytes.try_into().unwrap()), expected);
    }
}
