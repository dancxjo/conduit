//! Profiles limit physical storage without changing Source Type contracts.
use conduit_plot::{rust_binding::*, *};

const SOURCE: &str = r#"
type Label = Text <= 1024B
type Records = {
    labels: sequence Label <= 8
}
type Payload =
    quiet
    | records Records
type Row = {
    value: Payload?
}
type Batch = {
    rows: sequence Row <= 4
}
"#;
fn schema() -> PreparedStorageSchema {
    let doc =
        check_syntax_document(&parse_syntax_document(SOURCE), &StartupCatalog::new()).unwrap();
    let root = &doc
        .native_types
        .iter()
        .find(|n| n.name == "Batch")
        .unwrap()
        .identity;
    discover_prepared_storage_schema(&doc.native_types, root).unwrap()
}
fn request(schema: &PreparedStorageSchema) -> PreparedStorageProfile {
    PreparedStorageProfile {
        root: schema.root().clone(),
        capacities: schema
            .sites()
            .iter()
            .filter(|s| matches!(s.limit, PreparedStorageLimit::SequenceItems(_)))
            .cloned()
            .collect(),
    }
}

#[test]
fn complete_typed_paths_cover_nested_optional_payload_and_text_without_expansion() {
    let schema = schema();
    assert!(
        schema.unsupported().is_empty(),
        "{:?}",
        schema.unsupported()
    );
    assert_eq!(schema.sites().len(), 3);
    let text = schema
        .sites()
        .iter()
        .find(|s| matches!(s.limit, PreparedStorageLimit::TextBytes(_)))
        .unwrap();
    assert_eq!(text.limit, PreparedStorageLimit::TextBytes(1024));
    assert!(text.path.contains(&NativePathStep::Each));
    assert!(text.path.contains(&NativePathStep::Variant("some".into())));
    assert!(text
        .path
        .contains(&NativePathStep::Variant("records".into())));
    assert!(text.path.contains(&NativePathStep::Representation));
    let original = schema.clone();
    let mut profile = request(&schema);
    for c in &mut profile.capacities {
        c.limit = PreparedStorageLimit::SequenceItems(0);
    }
    profile.capacities.push(PreparedStorageCapacity {
        path: text.path.clone(),
        limit: PreparedStorageLimit::TextBytes(16),
    });
    let resolved = resolve_prepared_storage_profile(&schema, &profile).unwrap();
    assert_eq!(resolved.capacities.len(), 3);
    assert!(resolved.capacities.iter().all(|s| matches!(
        s.limit,
        PreparedStorageLimit::SequenceItems(0) | PreparedStorageLimit::TextBytes(16)
    )));
    assert_eq!(schema, original);
}

#[test]
fn absent_text_override_keeps_semantic_bound_but_missing_sequence_count_refuses() {
    let schema = schema();
    let mut profile = request(&schema);
    let resolved = resolve_prepared_storage_profile(&schema, &profile).unwrap();
    assert!(resolved
        .capacities
        .iter()
        .any(|s| s.limit == PreparedStorageLimit::TextBytes(1024)));
    let omitted = profile.capacities.pop().unwrap();
    assert_eq!(
        resolve_prepared_storage_profile(&schema, &profile),
        Err(PreparedStorageProfileRefusal::MissingSequenceCapacity(
            omitted.path
        ))
    );
}

#[test]
fn foreign_duplicate_unknown_mismatched_and_excess_profiles_refuse() {
    let schema = schema();
    let original = request(&schema);
    let mut profile = original.clone();
    profile.root = conduit_core::KindId::from("fixture/foreign");
    assert_eq!(
        resolve_prepared_storage_profile(&schema, &profile),
        Err(PreparedStorageProfileRefusal::ForeignRoot)
    );
    profile = original.clone();
    profile.capacities.push(profile.capacities[0].clone());
    assert!(matches!(
        resolve_prepared_storage_profile(&schema, &profile),
        Err(PreparedStorageProfileRefusal::DuplicatePath(_))
    ));
    profile = original.clone();
    profile.capacities[0].path = vec![NativePathStep::Field("absent".into())];
    assert!(matches!(
        resolve_prepared_storage_profile(&schema, &profile),
        Err(PreparedStorageProfileRefusal::UnknownPath(_))
    ));
    profile = original.clone();
    profile.capacities[0].limit = PreparedStorageLimit::TextBytes(1);
    assert!(matches!(
        resolve_prepared_storage_profile(&schema, &profile),
        Err(PreparedStorageProfileRefusal::WrongShape(_))
    ));
    profile = original;
    profile.capacities[0].limit = PreparedStorageLimit::SequenceItems(9);
    assert!(matches!(
        resolve_prepared_storage_profile(&schema, &profile),
        Err(PreparedStorageProfileRefusal::CapacityExceeded(_))
    ));
}

#[test]
fn text_capacity_and_path_work_cannot_exceed_their_declared_limits() {
    let schema = schema();
    let mut profile = request(&schema);
    let mut text = schema
        .sites()
        .iter()
        .find(|s| matches!(s.limit, PreparedStorageLimit::TextBytes(_)))
        .unwrap()
        .clone();
    text.limit = PreparedStorageLimit::TextBytes(1025);
    profile.capacities.push(text);
    assert!(matches!(
        resolve_prepared_storage_profile(&schema, &profile),
        Err(PreparedStorageProfileRefusal::CapacityExceeded(_))
    ));
    let mut profile = request(&schema);
    profile.capacities[0].path = vec![NativePathStep::Each; 65];
    assert_eq!(
        resolve_prepared_storage_profile(&schema, &profile),
        Err(PreparedStorageProfileRefusal::Bounds)
    );
    assert_eq!(
        discover_prepared_storage_schema(&[], schema.root()),
        Err(PreparedStorageProfileRefusal::MissingRoot)
    );
}

#[test]
fn unsupported_schema_is_reported_at_its_exact_path_and_cannot_be_resolved() {
    let doc = check_syntax_document(
        &parse_syntax_document(
            "type Inner = {\n n: U8\n}\ntype Root = {\n values: collection Inner = 2\n}\n",
        ),
        &StartupCatalog::new(),
    )
    .unwrap();
    let root = &doc
        .native_types
        .iter()
        .find(|n| n.name == "Root")
        .unwrap()
        .identity;
    let schema = discover_prepared_storage_schema(&doc.native_types, root).unwrap();
    assert_eq!(schema.unsupported().len(), 1);
    assert_eq!(
        schema.unsupported()[0].path,
        vec![NativePathStep::Field("values".into())]
    );
    assert_eq!(
        schema.unsupported()[0].reason,
        "nonprimitive/large fixed array"
    );
    assert_eq!(
        resolve_prepared_storage_profile(&schema, &request(&schema)),
        Err(PreparedStorageProfileRefusal::UnsupportedSchema)
    );
}
