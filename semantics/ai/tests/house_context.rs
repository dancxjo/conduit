use conduit_ai::{
    build_house_model_request, wired_house_context_item, HouseContextProvenanceClass,
    HouseContextRefusal, WiredHouseContextItem, MAXIMUM_HOUSE_CONTEXT_BYTES,
    MAXIMUM_HOUSE_CONTEXT_ITEMS,
};
use conduit_form::rust_binding::NativeRustBinding;

fn item(identity: &str, provenance: HouseContextProvenanceClass) -> WiredHouseContextItem {
    wired_house_context_item(
        identity,
        "temperature/summary@1",
        b"upstairs: 21 C",
        provenance,
        "sign/temperature-reading/42",
    )
    .unwrap()
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

    assert_eq!(request.context().as_slice(), &[wired]);
    assert_eq!(*request.maximum_output_bytes(), 1024);
    assert!(request
        .request_identity()
        .starts_with("house-model-request/"));
    assert!(!request.request_identity().contains("ollama"));
}

#[test]
fn provenance_classes_remain_distinct_and_affect_exact_identity() {
    let observed = item("context/fact", HouseContextProvenanceClass::ObservedSign);
    let history = item(
        "context/fact",
        HouseContextProvenanceClass::ModelDerivedHistory,
    );

    let first = build_house_model_request("status?", &[observed], 512).unwrap();
    let second = build_house_model_request("status?", &[history], 512).unwrap();
    assert_ne!(first.request_identity(), second.request_identity());
}

#[test]
fn complete_house_request_round_trips_through_its_native_owner() {
    let request = build_house_model_request(
        "status?",
        &[item(
            "context/fact",
            HouseContextProvenanceClass::DeclaredConfiguration,
        )],
        512,
    )
    .unwrap();

    let structured = request.clone().into_structured().unwrap();
    assert_eq!(
        conduit_ai::HouseModelRequest::from_structured(structured).unwrap(),
        request
    );
    assert!(!include_str!("../src/house_context.rs")
        .contains(concat!("pub struct ", "WiredHouseContextItem")));
    assert!(!include_str!("../src/house_context.rs")
        .contains(concat!("pub struct ", "HouseModelRequest")));
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

    assert_eq!(
        wired_house_context_item(
            "context/large",
            "temperature/summary@1",
            &vec![b'x'; MAXIMUM_HOUSE_CONTEXT_BYTES + 1],
            HouseContextProvenanceClass::ObservedSign,
            "sign/temperature/42",
        ),
        Err(HouseContextRefusal::ContextByteLimitExceeded)
    );

    let half = vec![b'x'; MAXIMUM_HOUSE_CONTEXT_BYTES / 2 + 1];
    let aggregate = [
        wired_house_context_item(
            "context/large-a",
            "temperature/summary@1",
            &half,
            HouseContextProvenanceClass::ObservedSign,
            "sign/temperature/42",
        )
        .unwrap(),
        wired_house_context_item(
            "context/large-b",
            "temperature/summary@1",
            &half,
            HouseContextProvenanceClass::ObservedSign,
            "sign/temperature/43",
        )
        .unwrap(),
    ];
    assert_eq!(
        build_house_model_request("status?", &aggregate, 512),
        Err(HouseContextRefusal::ContextByteLimitExceeded)
    );

    let duplicate = item("context/same", HouseContextProvenanceClass::ObservedSign);
    assert_eq!(
        build_house_model_request("status?", &[duplicate.clone(), duplicate], 512),
        Err(HouseContextRefusal::DuplicateItem)
    );
}
