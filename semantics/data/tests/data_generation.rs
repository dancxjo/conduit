use conduit_core::{data_reference_kind, kind_id, ResourceClassId, ValuePayload};
use conduit_data::{
    data_load_text_projection, data_save_text_projection, DataGenerationRefusal,
    DataGenerationStore, DataReference, DataReferenceRefusal,
};

fn text(value: &str) -> ValuePayload {
    ValuePayload {
        value_kind: kind_id("value/text"),
        encoded: value.as_bytes().to_vec(),
    }
}

#[test]
fn published_generation_remains_a_after_current_value_evolves_to_b() {
    let mut store = DataGenerationStore::new(kind_id("value/text"), 4, 32).unwrap();
    let generation_a = store.publish(text("A")).unwrap();
    let current = text("B");

    assert_eq!(store.load(&generation_a).unwrap(), text("A"));
    assert_eq!(current, text("B"));
    assert_eq!(
        generation_a.to_value().unwrap().value_kind,
        data_reference_kind(&kind_id("value/text"))
    );
}

#[test]
fn generations_are_distinct_immutable_and_independently_loadable() {
    let mut store = DataGenerationStore::new(kind_id("value/text"), 3, 16).unwrap();
    let first = store.publish(text("same")).unwrap();
    let second = store.publish(text("same")).unwrap();

    assert_ne!(first, second);
    assert_eq!(first.reference().identity, second.reference().identity);
    assert_ne!(
        first.reference().lifetime.version,
        second.reference().lifetime.version
    );
    assert_eq!(store.load(&first).unwrap(), text("same"));
    assert_eq!(store.load(&second).unwrap(), text("same"));
}

#[test]
fn wrong_kind_malformed_reference_and_capacity_refuse_without_mutation() {
    let mut store = DataGenerationStore::new(kind_id("value/text"), 1, 4).unwrap();
    assert_eq!(
        store.publish(ValuePayload {
            value_kind: kind_id("value/bytes"),
            encoded: vec![1]
        }),
        Err(DataGenerationRefusal::WrongContentKind)
    );
    assert_eq!(store.generation_count(), 0);
    assert_eq!(
        store.publish(text("oversize")),
        Err(DataGenerationRefusal::ValueTooLarge)
    );
    assert_eq!(store.generation_count(), 0);

    let retained = store.publish(text("A")).unwrap();
    assert_eq!(
        store.publish(text("B")),
        Err(DataGenerationRefusal::GenerationCapacityExhausted)
    );
    assert_eq!(store.generation_count(), 1);
    assert_eq!(store.load(&retained).unwrap(), text("A"));

    let mut wrong_extent = retained.reference().clone();
    wrong_extent.extent.bytes = 2;
    let wrong_extent = DataReference::new(wrong_extent).unwrap();
    assert_eq!(
        store.load(&wrong_extent),
        Err(DataGenerationRefusal::ExtentMismatch)
    );
    assert_eq!(
        DataReference::decode_for(&kind_id("value/text"), b"/tmp/not-data"),
        Err(DataReferenceRefusal::Malformed)
    );
}

#[test]
fn save_and_load_fronts_join_only_through_exact_text_reference_kind() {
    let save = data_save_text_projection();
    let load = data_load_text_projection();
    let reference_kind = data_reference_kind(&kind_id("value/text"));

    assert_eq!(save.inputs[0].value_kind, kind_id("value/text"));
    assert_eq!(save.outputs[0].value_kind, reference_kind);
    assert_eq!(save.outputs[0].value_kind, load.inputs[0].value_kind);
    assert_eq!(save.outputs[0].temporal, load.inputs[0].temporal);
    assert_eq!(load.outputs[0].value_kind, kind_id("value/text"));
    assert_ne!(
        save.outputs[0].value_kind,
        data_reference_kind(&kind_id("value/bytes"))
    );
}

#[test]
fn reference_value_refuses_wrong_type_content_and_access_class() {
    let mut store = DataGenerationStore::new(kind_id("value/text"), 1, 8).unwrap();
    let reference = store.publish(text("A")).unwrap();
    let value = reference.to_value().unwrap();

    assert_eq!(
        DataReference::from_value_for(&kind_id("value/bytes"), &value),
        Err(DataReferenceRefusal::WrongReferenceKind)
    );
    assert_eq!(
        DataReference::decode_for(&kind_id("value/bytes"), &value.encoded),
        Err(DataReferenceRefusal::WrongContentKind)
    );

    let mut foreign = reference.reference().clone();
    foreign.access_class = ResourceClassId::from("filesystem/readable");
    assert_eq!(
        DataReference::new(foreign),
        Err(DataReferenceRefusal::WrongAccessClass)
    );
}
