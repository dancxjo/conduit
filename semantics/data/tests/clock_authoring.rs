use conduit_data::{clock_authoring, ClockRelation, ClockRelationQuality};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};

#[test]
fn narrow_clock_import_preserves_owner_roots_and_nested_refinements() {
    let checked = clock_authoring::checked_types().unwrap();
    assert_eq!(checked.len(), 2);
    let relation = checked
        .iter()
        .find(|ty| ty.name == "ClockRelation")
        .unwrap();
    let quality = checked
        .iter()
        .find(|ty| ty.name == "ClockRelationQuality")
        .unwrap();
    assert_eq!(relation.value_type, ClockRelation::semantic_type().unwrap());
    assert_eq!(
        quality.value_type,
        ClockRelationQuality::semantic_type().unwrap()
    );
    assert!(!relation.value_contracts.is_empty());
    assert!(!quality.value_contracts.is_empty());
    let mut catalog = StartupCatalog::new();
    for ty in &checked {
        catalog
            .insert_checked_native_type(ty.name.clone(), ty)
            .unwrap();
    }
    let imported = check_syntax_document(
        &parse_syntax_document("type ClockEnvelope = { relation: ClockRelation }"),
        &catalog,
    )
    .unwrap();
    let envelope = imported
        .native_types
        .iter()
        .find(|ty| ty.name == "ClockEnvelope")
        .unwrap();
    for contract in &relation.value_contracts {
        let path = format!(".relation{}", contract.representation_path);
        assert!(
            envelope
                .value_contracts
                .iter()
                .any(|nested| nested.representation_path == path
                    && nested.contract == contract.contract),
            "lost imported contract {path}"
        );
    }
}
