use conduit_ai::{HouseContextProvenanceClass, WiredHouseContextItem};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_text::AddressDetection;
use conduit_tongues::{
    decode_address_detection, decode_wired_house_context, encode_address_detection,
    encode_wired_house_context, house_prompt_contract, install_house_conversation_catalog,
    prepare_house_generation_prompt, HouseConversationValueError, HousePromptRefusal,
    HOUSE_CONTEXT_TO_PROMPT_KIND,
};

fn context(provenance: HouseContextProvenanceClass) -> WiredHouseContextItem {
    WiredHouseContextItem {
        item_identity: "context/upstairs-temperature".into(),
        value_kind: "temperature/summary@1".into(),
        canonical_value: b"21 C, observed 18 seconds ago".to_vec(),
        provenance,
        source_identity: "sign/temperature/42".into(),
    }
}

#[test]
fn runtime_port_values_are_canonical_bounded_and_provider_neutral() {
    let detection = AddressDetection::Addressed {
        matched_name_index: 0,
        utterance: "status upstairs?".into(),
    };
    let detection_bytes = encode_address_detection(&detection).unwrap();
    assert_eq!(decode_address_detection(&detection_bytes), Ok(detection));
    let context = vec![context(HouseContextProvenanceClass::ObservedSign)];
    let context_bytes = encode_wired_house_context(&context).unwrap();
    assert_eq!(decode_wired_house_context(&context_bytes), Ok(context));
    let text = String::from_utf8(context_bytes).unwrap();
    for forbidden in ["ollama", "localhost", "11434", "model_name"] {
        assert!(!text.contains(forbidden));
    }
}

#[test]
fn malformed_noncanonical_and_invalid_port_values_refuse() {
    assert_eq!(
        decode_address_detection(
            br#"{ "schema":"conduit.house/address-detection-value@1","status":"not-addressed","matched_name_index":null,"utterance":null}"#
        ),
        Err(HouseConversationValueError::NonCanonical)
    );
    assert_eq!(
        decode_address_detection(b"not-json"),
        Err(HouseConversationValueError::Malformed)
    );
    let invalid = AddressDetection::Addressed {
        matched_name_index: conduit_text::MAX_ADDRESS_NAMES as u8,
        utterance: "status".into(),
    };
    assert_eq!(
        encode_address_detection(&invalid),
        Err(HouseConversationValueError::InvalidValue)
    );
    assert_eq!(
        decode_wired_house_context(br#"{"schema":"wrong","items":[]}"#),
        Err(HouseConversationValueError::WrongSchema)
    );
}

#[test]
fn addressed_house_context_becomes_bounded_provider_neutral_model_input() {
    let prompt = prepare_house_generation_prompt(
        &AddressDetection::Addressed {
            matched_name_index: 0,
            utterance: "what is the temperature upstairs?".into(),
        },
        &[context(HouseContextProvenanceClass::ObservedSign)],
        1024,
    )
    .unwrap();
    assert!(prompt.prompt.contains("what is the temperature upstairs?"));
    assert!(prompt.prompt.contains("21 C, observed 18 seconds ago"));
    assert!(prompt.prompt.contains("observed-sign"));
    assert!(prompt.prompt.contains("sign/temperature/42"));
    for forbidden in ["ollama", "localhost", "11434", "model_name", "HostId"] {
        assert!(!prompt.prompt.contains(forbidden));
    }
}

#[test]
fn unaddressed_text_never_becomes_model_eligible() {
    assert_eq!(
        prepare_house_generation_prompt(
            &AddressDetection::NotAddressed,
            &[context(HouseContextProvenanceClass::ObservedSign)],
            1024,
        ),
        Err(HousePromptRefusal::NotAddressed)
    );
}

#[test]
fn canonical_house_conversation_is_an_ordinary_checked_form() {
    let source = include_str!("../../../forms/house-conversation/main.conduit");
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_text::install_text_catalogs(&mut startup, &mut profile).unwrap();
    conduit_ai::install_generate_text_catalog(&mut startup, &mut profile).unwrap();
    install_house_conversation_catalog(&mut startup, &mut profile).unwrap();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authored =
        expand_canonical_form_for_authoring(&checked, "house-conversation", &profile).unwrap();
    let expanded = authored.expanded;
    assert_eq!(authored.input_bindings.len(), 2);
    assert_eq!(authored.output_bindings.len(), 1);
    assert_eq!(expanded.gears.len(), 2);
    assert!(expanded
        .gears
        .iter()
        .any(|gear| gear.kind_id.as_str() == HOUSE_CONTEXT_TO_PROMPT_KIND));
    assert!(expanded
        .gears
        .iter()
        .any(|gear| gear.kind_id.as_str() == "ai/generate-text"));
    let contract = house_prompt_contract();
    assert_eq!(contract.inputs.len(), 2);
    for forbidden in ["ollama", "http", "microphone", "speaker", "actuator"] {
        assert!(!source.to_ascii_lowercase().contains(forbidden));
    }
}
