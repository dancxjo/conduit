use super::*;

#[test]
fn put_get_delete_and_missing_are_exact() {
    let pattern = conduit_semantic_catalog::normalized_value(&[250_000, 1_000_000]).unwrap();
    let put = conduit_semantic_catalog::put_template_command("cadence", pattern.clone())
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let get = conduit_semantic_catalog::get_template_command("cadence")
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let delete = conduit_semantic_catalog::delete_template_command("cadence")
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let mut host = TemplateStorageHost::prepare();
    assert_eq!(
        host.execute(&put).unwrap(),
        conduit_semantic_catalog::stored_template_result("cadence")
            .unwrap()
            .canonical_bytes()
            .unwrap()
    );
    assert_eq!(
        host.execute(&get).unwrap(),
        conduit_semantic_catalog::found_template_result("cadence", pattern)
            .unwrap()
            .canonical_bytes()
            .unwrap()
    );
    assert_eq!(host.execute(&delete).unwrap(), deleted("cadence"));
    assert_eq!(
        host.execute(&get).unwrap(),
        conduit_semantic_catalog::missing_template_result("cadence")
            .unwrap()
            .canonical_bytes()
            .unwrap()
    );
}

#[test]
fn full_duplicate_and_corrupt_retained_storage_are_distinct() {
    let pattern = conduit_semantic_catalog::normalized_value(&[1]).unwrap();
    let mut host = TemplateStorageHost::prepare();
    let first = command("slot-0", pattern.clone());
    host.execute(&first).unwrap();
    assert_eq!(host.execute(&first), Err(StorageRefusal::DuplicateName));
    for index in 1..SLOT_COUNT {
        host.execute(&command(&format!("slot-{index}"), pattern.clone()))
            .unwrap();
    }
    assert_eq!(
        host.execute(&command("overflow", pattern)),
        Err(StorageRefusal::Full)
    );
    host.slots[0].pattern_node[0] = 99;
    let get = conduit_semantic_catalog::get_template_command("slot-0")
        .unwrap()
        .canonical_bytes()
        .unwrap();
    assert_eq!(
        host.execute(&get),
        Err(StorageRefusal::CorruptRetainedTemplate)
    );
}

fn command(name: &str, pattern: conduit_core::StructuredInfoValue) -> Vec<u8> {
    conduit_semantic_catalog::put_template_command(name, pattern)
        .unwrap()
        .canonical_bytes()
        .unwrap()
}

fn deleted(name: &str) -> Vec<u8> {
    let name_type = conduit_core::StructuredInfoType::leaf(conduit_core::kind_id(
        conduit_semantic_catalog::TEMPLATE_NAME_INFO_ID,
    ))
    .unwrap();
    let name =
        conduit_core::StructuredInfoValue::leaf(name_type, name.as_bytes().to_vec()).unwrap();
    conduit_core::StructuredInfoValue::variant(
        conduit_semantic_catalog::template_storage_result_type(),
        "deleted",
        name,
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
