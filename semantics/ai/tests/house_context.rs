use conduit_ai::{
    build_house_model_request, HouseContextProvenanceClass, HouseContextRefusal,
    WiredHouseContextItem, MAXIMUM_HOUSE_CONTEXT_BYTES, MAXIMUM_HOUSE_CONTEXT_ITEMS,
};

fn item(identity: &str, provenance: HouseContextProvenanceClass) -> WiredHouseContextItem {
    WiredHouseContextItem {
        item_identity: identity.into(),
        value_kind: "temperature/summary@1".into(),
        canonical_value: b"upstairs: 21 C".to_vec(),
        provenance,
        source_identity: "sign/temperature-reading/42".into(),
    }
}

#[test]
fn admits_only_the_context_explicitly_supplied_by_the_form() {
    let wired = item(
        "context/upstairs-temperature",
        HouseContextProvenanceClass::ObservedSign,
    );
    let request = build_house_model_request(
        "what is the temperature upstairs?",
        core::slice::from_ref(&wired),
        1024,
    )
    .expect("one wired fact is finite");

    assert_eq!(request.context, vec![wired]);
    assert_eq!(request.maximum_output_bytes, 1024);
    assert!(request.request_identity.starts_with("house-model-request/"));
    assert!(!request.request_identity.contains("ollama"));
}

#[test]
fn provenance_classes_remain_distinct_and_affect_exact_identity() {
    let observed = item("context/fact", HouseContextProvenanceClass::ObservedSign);
    let mut history = observed.clone();
    history.provenance = HouseContextProvenanceClass::ModelDerivedHistory;

    let first = build_house_model_request("status?", &[observed], 512).unwrap();
    let second = build_house_model_request("status?", &[history], 512).unwrap();
    assert_ne!(first.request_identity, second.request_identity);
}

#[test]
fn refuses_missing_implicit_or_unbounded_context() {
    assert_eq!(
        build_house_model_request("status?", &[], 512),
        Err(HouseContextRefusal::EmptyContext)
    );

    let too_many = (0..=MAXIMUM_HOUSE_CONTEXT_ITEMS)
        .map(|index| {
            item(
                &format!("context/{index}"),
                HouseContextProvenanceClass::DeclaredConfiguration,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        build_house_model_request("status?", &too_many, 512),
        Err(HouseContextRefusal::ContextItemLimitExceeded)
    );

    let mut oversized = item("context/large", HouseContextProvenanceClass::ObservedSign);
    oversized.canonical_value = vec![b'x'; MAXIMUM_HOUSE_CONTEXT_BYTES + 1];
    assert_eq!(
        build_house_model_request("status?", &[oversized], 512),
        Err(HouseContextRefusal::ContextByteLimitExceeded)
    );

    let duplicate = item("context/same", HouseContextProvenanceClass::ObservedSign);
    assert_eq!(
        build_house_model_request("status?", &[duplicate.clone(), duplicate], 512),
        Err(HouseContextRefusal::DuplicateItem)
    );
}
