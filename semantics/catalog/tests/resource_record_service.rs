//! Authored storage operations retain the original physical contract types.
use conduit_plot::*;
#[test]
fn authored_record_service_keeps_both_typed_fore_pairs_and_terminal_laws() {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_semantic_catalog::install_resource_record_service_catalog(&mut startup, &mut profiles)
        .unwrap();
    let source = r#"
plot history-store (
    >> record: Bytes...| <= 200B
    >> request: Count...|
    append-status: RecordAppendResult...| >>
    read-result: RecordReadResult...| <= READ_RESULT_BOUND B >>
) {
    store: resource/record-service
    record >> store.record
    request >> store.read-index
    store.index >> append-status
    store.read-result >> read-result
}
"#;
    let source = source.replace(
        "READ_RESULT_BOUND B",
        &format!(
            "{}B",
            conduit_resource_snapshot::read_result::READ_RESULT_BYTES
        ),
    );
    let syntax = parse_syntax_document(&source);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "history-store", &profiles).unwrap();
    assert_eq!(expanded.expanded.gears.len(), 1);
    let gear = &expanded.expanded.gears[0];
    let original = conduit_resource_snapshot::service_contract::definition();
    assert_eq!(gear.kind_id, original.kind_id);
    assert_eq!(gear.inputs, original.inputs);
    assert_eq!(gear.outputs, original.outputs);
    assert_eq!(gear.semantic_contract.laws, original.semantic_laws);
    assert_eq!(expanded.input_bindings.len(), 2);
    assert_eq!(expanded.output_bindings.len(), 2);
}
