use conduit_core::{
    data_reference_kind, kind_id, port_id, FrontValueLocation, ResourceClassId, ValuePayload,
};
use conduit_data::{
    data_load_text_contract, data_load_text_projection, data_save_text_contract,
    data_save_text_projection, DataGenerationNamespace, DataGenerationNamespaceRefusal,
    DataGenerationRefusal, DataGenerationStore, DataGenerationTextValue, DataLoadTextTerminal,
    DataReference, DataReferenceRefusal, DataSaveTextTerminal, PreparedDataGenerationStore,
    DATA_LOAD_TEXT_TERMINAL_INFO_ID, DATA_SAVE_TEXT_TERMINAL_INFO_ID, DATA_TEXT_CONTRACT_REVISION,
    DATA_TEXT_TERMINAL_ENCODED_LEN, MAXIMUM_DATA_GENERATION_NAMESPACE_BYTES,
    MAXIMUM_DATA_REFERENCE_ENCODED_BYTES, MAXIMUM_DATA_TEXT_BYTES,
};

fn namespace(name: &str) -> DataGenerationNamespace {
    DataGenerationNamespace::new(name).unwrap()
}

#[test]
fn generated_generic_generation_value_preserves_concrete_text_meaning() {
    let value = DataGenerationTextValue::new(namespace("notes"), "portable".into()).unwrap();
    assert_eq!(value.value(), "portable");
    assert_eq!(value.namespace().digest(), namespace("notes").digest());
    let encoded = conduit_form::rust_binding::NativeRustBinding::encode(value.clone()).unwrap();
    assert_eq!(
        <DataGenerationTextValue as conduit_form::rust_binding::NativeRustBinding>::decode(
            &encoded
        )
        .unwrap(),
        value
    );
    assert!(DataGenerationTextValue::new(namespace("notes"), "x".repeat(4097)).is_err());
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

#[test]
fn prepared_store_publishes_and_loads_without_growing_storage() {
    let kind = kind_id("value/text");
    let reference_bytes =
        conduit_data::maximum_data_reference_encoded_bytes(kind.as_str()).unwrap();
    let mut store =
        PreparedDataGenerationStore::new(namespace("test/prepared"), kind.clone(), 2, 8, 16)
            .unwrap();
    let capacities = store.allocation_capacities();
    let mut first = Vec::with_capacity(reference_bytes);
    let mut second = Vec::with_capacity(reference_bytes);
    let first_capacity = first.capacity();
    let second_capacity = second.capacity();

    store.publish_into(&kind, b"same", &mut first).unwrap();
    store.publish_into(&kind, b"same", &mut second).unwrap();

    assert_ne!(first, second);
    assert_eq!(store.load_encoded(&first), Ok(b"same".as_slice()));
    assert_eq!(store.load_encoded(&second), Ok(b"same".as_slice()));
    assert_eq!(store.generation_count(), 2);
    assert_eq!(store.retained_bytes(), 8);
    assert_eq!(store.allocation_capacities(), capacities);
    assert_eq!(first.capacity(), first_capacity);
    assert_eq!(second.capacity(), second_capacity);
}

#[test]
fn prepared_store_refuses_pressure_atomically_and_keeps_namespaces_distinct() {
    let kind = kind_id("value/text");
    let reference_bytes =
        conduit_data::maximum_data_reference_encoded_bytes(kind.as_str()).unwrap();
    let mut first =
        PreparedDataGenerationStore::new(namespace("test/prepared/first"), kind.clone(), 2, 4, 4)
            .unwrap();
    let mut second =
        PreparedDataGenerationStore::new(namespace("test/prepared/second"), kind.clone(), 2, 4, 4)
            .unwrap();
    let mut first_reference = Vec::with_capacity(reference_bytes);
    let mut second_reference = Vec::with_capacity(reference_bytes);
    first
        .publish_into(&kind, b"same", &mut first_reference)
        .unwrap();
    second
        .publish_into(&kind, b"same", &mut second_reference)
        .unwrap();
    assert_ne!(first_reference, second_reference);

    let mut output = Vec::with_capacity(reference_bytes);
    assert_eq!(
        first.publish_into(&kind, b"x", &mut output),
        Err(DataGenerationRefusal::ByteCapacityExhausted)
    );
    assert_eq!(first.generation_count(), 1);
    assert_eq!(first.retained_bytes(), 4);
    assert_eq!(first.load_encoded(&first_reference), Ok(b"same".as_slice()));
    assert_eq!(
        first.load_encoded(&second_reference),
        Err(DataGenerationRefusal::GenerationNotRetained)
    );
}

#[test]
fn prepared_store_requires_prepared_reference_output() {
    let kind = kind_id("value/text");
    let mut store =
        PreparedDataGenerationStore::new(namespace("test/prepared"), kind.clone(), 1, 8, 8)
            .unwrap();
    let mut unprepared = Vec::new();

    assert_eq!(
        store.publish_into(&kind, b"A", &mut unprepared),
        Err(DataGenerationRefusal::ReferenceOutputCapacity)
    );
    assert_eq!(store.generation_count(), 0);
    assert_eq!(store.retained_bytes(), 0);
}
