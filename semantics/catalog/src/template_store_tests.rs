use super::*;
use alloc::format;

#[test]
fn oversized_command_refuses_without_changing_retained_slots() {
    let mut store = BoundedTemplateStore::prepare();
    let capacities: Vec<_> = store
        .slots
        .iter()
        .map(|slot| slot.pattern_node.capacity())
        .collect();
    assert_eq!(
        store.execute(&alloc::vec![0; MAXIMUM_STRUCTURED_CANONICAL_BYTES + 1]),
        Err(TemplateStoreRefusal::Malformed)
    );
    assert!(store.slots.iter().all(|slot| !slot.occupied));
    assert_eq!(
        capacities,
        store
            .slots
            .iter()
            .map(|slot| slot.pattern_node.capacity())
            .collect::<Vec<_>>()
    );
}

#[test]
fn put_get_delete_and_missing_are_exact() {
    let pattern = crate::normalized_value(&[250_000, 1_000_000]).unwrap();
    let put = crate::put_template_command("cadence", pattern.clone())
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let get = crate::get_template_command("cadence")
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let delete = crate::delete_template_command("cadence")
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let mut host = BoundedTemplateStore::prepare();
    assert_eq!(
        host.execute(&put).unwrap(),
        crate::stored_template_result("cadence")
            .unwrap()
            .canonical_bytes()
            .unwrap()
    );
    assert_eq!(
        host.execute(&get).unwrap(),
        crate::found_template_result("cadence", pattern)
            .unwrap()
            .canonical_bytes()
            .unwrap()
    );
    assert_eq!(host.execute(&delete).unwrap(), deleted("cadence"));
    assert_eq!(
        host.execute(&get).unwrap(),
        crate::missing_template_result("cadence")
            .unwrap()
            .canonical_bytes()
            .unwrap()
    );
}

#[test]
fn full_duplicate_and_corrupt_retained_storage_are_distinct() {
    let pattern = crate::normalized_value(&[1]).unwrap();
    let mut host = BoundedTemplateStore::prepare();
    let first = command("slot-0", pattern.clone());
    host.execute(&first).unwrap();
    assert_eq!(
        host.execute(&first),
        Err(TemplateStoreRefusal::DuplicateName)
    );
    for index in 1..SLOT_COUNT {
        host.execute(&command(&format!("slot-{index}"), pattern.clone()))
            .unwrap();
    }
    assert_eq!(
        host.execute(&command("overflow", pattern)),
        Err(TemplateStoreRefusal::Full)
    );
    host.slots[0].pattern_node[0] = 99;
    let get = crate::get_template_command("slot-0")
        .unwrap()
        .canonical_bytes()
        .unwrap();
    assert_eq!(
        host.execute(&get),
        Err(TemplateStoreRefusal::CorruptRetainedTemplate)
    );
}

fn command(name: &str, pattern: conduit_core::StructuredInfoValue) -> Vec<u8> {
    crate::put_template_command(name, pattern)
        .unwrap()
        .canonical_bytes()
        .unwrap()
}

fn deleted(name: &str) -> Vec<u8> {
    let name_type =
        conduit_core::StructuredInfoType::leaf(conduit_core::kind_id(crate::TEMPLATE_NAME_INFO_ID))
            .unwrap();
    let name =
        conduit_core::StructuredInfoValue::leaf(name_type, name.as_bytes().to_vec()).unwrap();
    conduit_core::StructuredInfoValue::variant(
        crate::template_storage_result_type(),
        "deleted",
        name,
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
