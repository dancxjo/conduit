use conduit_core::{
    data_reference_kind, kind_id, port_id, FrontValueLocation, ResourceClassId, ValuePayload,
};
use conduit_data::{
    data_load_text_contract, data_load_text_projection, data_save_text_contract,
    data_save_text_projection, DataGenerationNamespace, DataGenerationNamespaceRefusal,
    DataGenerationRefusal, DataGenerationStore, DataLoadTextTerminal, DataReference,
    DataReferenceRefusal, DataSaveTextTerminal, DATA_LOAD_TEXT_TERMINAL_INFO_ID,
    DATA_SAVE_TEXT_TERMINAL_INFO_ID, DATA_TEXT_CONTRACT_REVISION, DATA_TEXT_TERMINAL_ENCODED_LEN,
    MAXIMUM_DATA_GENERATION_NAMESPACE_BYTES, MAXIMUM_DATA_REFERENCE_ENCODED_BYTES,
    MAXIMUM_DATA_TEXT_BYTES,
};

fn namespace(name: &str) -> DataGenerationNamespace {
    DataGenerationNamespace::new(name).unwrap()
}

fn text(value: &str) -> ValuePayload {
    ValuePayload {
        value_kind: kind_id("value/text"),
        encoded: value.as_bytes().to_vec(),
    }
}

#[test]
fn published_generation_remains_a_after_current_value_evolves_to_b() {
    let mut store =
        DataGenerationStore::new(namespace("test/notebook"), kind_id("value/text"), 4, 32).unwrap();
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
    let mut store =
        DataGenerationStore::new(namespace("test/notebook"), kind_id("value/text"), 3, 16).unwrap();
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
    let mut store =
        DataGenerationStore::new(namespace("test/notebook"), kind_id("value/text"), 1, 4).unwrap();
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
    assert_eq!(save.inputs[0].abnormal_kind, None);
    assert_eq!(load.inputs[0].abnormal_kind, None);
    assert_eq!(
        save.outputs[0].abnormal_kind,
        Some(kind_id(DATA_SAVE_TEXT_TERMINAL_INFO_ID))
    );
    assert_eq!(
        load.outputs[0].abnormal_kind,
        Some(kind_id(DATA_LOAD_TEXT_TERMINAL_INFO_ID))
    );
    assert_ne!(
        save.outputs[0].value_kind,
        data_reference_kind(&kind_id("value/bytes"))
    );
}

#[test]
fn reference_value_refuses_wrong_type_content_and_access_class() {
    let mut store =
        DataGenerationStore::new(namespace("test/notebook"), kind_id("value/text"), 1, 8).unwrap();
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

#[test]
fn independent_publication_namespaces_never_alias_the_same_ordinal_content() {
    let mut first = DataGenerationStore::new(
        namespace("test/notebook/first"),
        kind_id("value/text"),
        1,
        8,
    )
    .unwrap();
    let mut second = DataGenerationStore::new(
        namespace("test/notebook/second"),
        kind_id("value/text"),
        1,
        8,
    )
    .unwrap();

    let first = first.publish(text("same")).unwrap();
    let second = second.publish(text("same")).unwrap();

    assert_eq!(first.reference().identity, second.reference().identity);
    assert_ne!(
        first.reference().lifetime.version,
        second.reference().lifetime.version
    );
}

#[test]
fn text_reference_has_one_exact_exported_bound() {
    let mut store =
        DataGenerationStore::new(namespace("test/notebook"), kind_id("value/text"), 1, 8).unwrap();
    let encoded = store.publish(text("A")).unwrap().encode().unwrap();
    let exact = conduit_data::maximum_data_reference_encoded_bytes("value/text").unwrap();

    assert_eq!(encoded.len(), exact);
    assert!(exact <= MAXIMUM_DATA_REFERENCE_ENCODED_BYTES);
}

#[test]
fn terminal_info_is_exact_and_save_and_load_are_not_interchangeable() {
    assert_eq!(DATA_TEXT_TERMINAL_ENCODED_LEN, 1);
    assert_eq!(
        DataSaveTextTerminal::decode(&DataSaveTextTerminal::ValueTooLarge.encode()),
        Ok(DataSaveTextTerminal::ValueTooLarge)
    );
    assert_eq!(
        DataLoadTextTerminal::decode(&DataLoadTextTerminal::GenerationNotRetained.encode()),
        Ok(DataLoadTextTerminal::GenerationNotRetained)
    );
    assert_ne!(
        kind_id(DATA_SAVE_TEXT_TERMINAL_INFO_ID),
        kind_id(DATA_LOAD_TEXT_TERMINAL_INFO_ID)
    );
}

#[test]
fn data_text_fronts_seal_payload_reference_and_abnormal_bounds() {
    let save = data_save_text_contract();
    let load = data_load_text_contract();
    save.validate().unwrap();
    load.validate().unwrap();
    assert_eq!(
        save.kind_contract_revision.as_str(),
        DATA_TEXT_CONTRACT_REVISION
    );

    let save = save.checked_front();
    let load = load.checked_front();
    let maximum = |front: &conduit_core::CheckedFront, location| {
        front
            .value_contract(&location)
            .expect("exact data value contract")
            .maximum_bytes
    };
    let reference_bytes =
        conduit_data::maximum_data_reference_encoded_bytes("value/text").unwrap() as u32;

    assert_eq!(
        maximum(&save, FrontValueLocation::Input(port_id("value"))),
        MAXIMUM_DATA_TEXT_BYTES
    );
    assert_eq!(
        maximum(&save, FrontValueLocation::Output(port_id("data"))),
        reference_bytes
    );
    assert_eq!(
        maximum(&save, FrontValueLocation::OutputAbnormal(port_id("data"))),
        DATA_TEXT_TERMINAL_ENCODED_LEN as u32
    );
    assert_eq!(
        maximum(&load, FrontValueLocation::Input(port_id("data"))),
        reference_bytes
    );
    assert_eq!(
        maximum(&load, FrontValueLocation::Output(port_id("value"))),
        MAXIMUM_DATA_TEXT_BYTES
    );
    assert_eq!(
        maximum(&load, FrontValueLocation::OutputAbnormal(port_id("value"))),
        DATA_TEXT_TERMINAL_ENCODED_LEN as u32
    );
}

#[test]
fn generation_namespace_is_explicit_and_bounded() {
    assert_eq!(
        DataGenerationNamespace::new(""),
        Err(DataGenerationNamespaceRefusal::Empty)
    );
    assert_eq!(
        DataGenerationNamespace::new(&"x".repeat(MAXIMUM_DATA_GENERATION_NAMESPACE_BYTES + 1)),
        Err(DataGenerationNamespaceRefusal::TooLarge)
    );
    assert_ne!(
        namespace("test/notebook/first"),
        namespace("test/notebook/second")
    );
}
