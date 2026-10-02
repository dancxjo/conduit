use conduit_core::{
    kind_id, BoundedResourceRef, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity,
};
use conduit_data::{
    deterministic_person_provider, deterministic_query_error, filter_active_rows,
    materialized_query_outcome, TabularColumnType, TabularPersonRowSlot, TabularQueryOutcomeFour,
    TabularQueryResultFour, TabularQueryStatus,
};
use conduit_plot::rust_binding::NativeRustBinding;

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

#[test]
fn materialized_outcome_round_trips_the_exact_resource_reference() {
    let reference = BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([1; 32]),
        content_profile: kind_id("tabular/result@1"),
        access_class: ResourceClassId::from("tabular/result"),
        extent: ResourceExtent {
            bytes: 128,
            items: Some(4),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([2; 32]),
            expires_at: None,
        },
    };
    let structured = materialized_query_outcome(&reference).expect("native outcome");
    assert_eq!(
        TabularQueryOutcomeFour::from_structured(structured).expect("round trip"),
        TabularQueryOutcomeFour::Materialized(reference)
    );
}
