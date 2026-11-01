//! Publication retirement preserves the last measurement; no radio timing claim.
use conduit_core::{
    kind_id, Quantity, StructuredFieldType as FieldType, StructuredFieldValue as Field,
    StructuredInfoType as Type, StructuredInfoValue as Value, StructuredVariantCase as Case, Unit,
};
use conduit_environment_http::{
    ap_ready, initialize, publish, response, set_ap_ready, set_ap_unavailable,
};
use conduit_face::FaceBasis;
#[allow(dead_code)]
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn body(response: &str) -> &str {
    response.split_once("\r\n\r\n").unwrap().1
}
// A canonical measurement packet at the public publication boundary. This
// fixture does not claim checked Source or device acquisition provenance.
fn observation() -> Vec<u8> {
    let quantity = Type::leaf(kind_id("value/quantity")).unwrap();
    let record = Type::record(
        kind_id("test/environment-observation"),
        vec![
            FieldType::new("temperature", quantity.clone()).unwrap(),
            FieldType::new("humidity", quantity.clone()).unwrap(),
        ],
    )
    .unwrap();
    let value = Value::record(
        record.clone(),
        vec![
            Field::new(
                "temperature",
                Value::leaf(
                    quantity.clone(),
                    Quantity::new(19, Unit::Celsius).encode().to_vec(),
                )
                .unwrap(),
            )
            .unwrap(),
            Field::new(
                "humidity",
                Value::leaf(quantity, Quantity::new(55, Unit::Percent).encode().to_vec()).unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let result = Type::variant(
        kind_id("test/environment-result"),
        vec![Case::new("observation", record).unwrap()],
    )
    .unwrap();
    Value::variant(result, "observation", value)
        .unwrap()
        .canonical_bytes()
        .unwrap()
}
#[test]
fn retiring_ap_changes_only_readiness_without_allocating_or_losing_measurement() {
    // This integration binary owns its fresh publication singleton. Early or
    // repeated cleanup must not manufacture readiness or require initialization.
    let (_, early_memory) = allocation_probe::observe(|| {
        set_ap_unavailable();
        set_ap_unavailable();
        assert!(!ap_ready());
    });
    assert_eq!(
        (early_memory.allocations, early_memory.reallocations),
        (0, 0)
    );
    initialize(FaceBasis {
        body_id: None,
        wake_id: None,
        source_document_id: Some("source/environment".into()),
        checked_plot_id: Some("checked/environment".into()),
        expanded_plot_id: Some("expanded/environment".into()),
        plan_id: Some("plan/environment".into()),
        active_play_id: Some("play/environment".into()),
        sign_ids: vec![],
    });
    publish(0, &observation(), 1000);
    set_ap_ready();
    assert!(ap_ready());
    let initial: serde_json::Value = serde_json::from_str(body(&response(1000, true))).unwrap();
    assert_eq!(initial["temperature_f"], 66.2);
    assert_eq!(initial["relative_humidity_percent"], 55);
    // Retain the actual last reading after a failed acquisition, as in the
    // recovered regression, while retiring only service availability.
    publish(1, &[], 2000);
    let mut expected: serde_json::Value =
        serde_json::from_str(body(&response(32000, true))).unwrap();
    assert_eq!(expected["sampled_at_boot_ms"], 1000);
    assert_eq!(expected["temperature_f"], 66.2);
    let (retired, memory) = allocation_probe::observe(|| {
        set_ap_unavailable();
        assert!(!ap_ready());
        response(32000, true)
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    expected["ap_ready"] = false.into();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(body(&retired)).unwrap(),
        expected
    );
}
