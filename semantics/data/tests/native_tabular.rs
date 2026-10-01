use conduit_data::{
    TabularColumnType, TabularPersonRowSlot, TabularQueryResultFour, TabularQueryStatus,
    deterministic_person_provider, deterministic_query_error, filter_active_rows,
};
use conduit_form::rust_binding::NativeRustBinding;

#[test]
fn finite_person_query_is_owned_by_the_native_tabular_type() {
    let value = deterministic_person_provider().expect("deterministic result");
    let result = TabularQueryResultFour::from_structured(value).expect("native result");

    assert_eq!(result.rows().get().len(), 4);
    assert!(matches!(
        result.rows().get()[3],
        TabularPersonRowSlot::Unused
    ));
    assert_eq!(result.schema().identity(), "tabular/person@1");
    assert!(matches!(
        result.schema().columns()[3].value_type(),
        TabularColumnType::OptionalText
    ));
    assert!(matches!(result.status(), TabularQueryStatus::Complete(_)));
}

#[test]
fn native_filter_preserves_errors_and_pads_filtered_rows() {
    let source = deterministic_person_provider().expect("deterministic result");
    let filtered = filter_active_rows(&source).expect("filtered result");
    let result = TabularQueryResultFour::from_structured(filtered).expect("native result");

    assert!(matches!(
        result.rows().get()[0],
        TabularPersonRowSlot::Row(_)
    ));
    assert!(matches!(
        result.rows().get()[1],
        TabularPersonRowSlot::Row(_)
    ));
    assert!(matches!(
        result.rows().get()[2],
        TabularPersonRowSlot::Unused
    ));
    assert!(matches!(
        result.rows().get()[3],
        TabularPersonRowSlot::Unused
    ));

    let error = deterministic_query_error("offline", "provider unavailable").expect("error");
    assert_eq!(filter_active_rows(&error).expect("preserved error"), error);
}
