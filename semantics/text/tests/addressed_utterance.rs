use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_text::{
    decode_address_detection, decode_address_set, encode_address_detection, encode_address_set,
    install_text_catalogs, AddressConfigurationError, AddressDetection, AddressDetectionError,
    AddressSet, AddressValueError, ADDRESS_DETECTION_VALUE_KIND, ADDRESS_DETECT_KIND,
    ADDRESS_SET_VALUE_KIND, MAX_ADDRESS_NAMES, MAX_ADDRESS_NAME_BYTES, MAX_TEXT_BYTES,
};

#[test]
fn primary_name_and_alias_produce_only_the_post_address_utterance() {
    let addresses = AddressSet::new(&["Rosehip House", "Rosehip"]).unwrap();
    assert_eq!(
        addresses
            .detect("  rosehip house, what's upstairs?")
            .unwrap(),
        AddressDetection::Addressed {
            matched_name_index: 0,
            utterance: "what's upstairs?".into(),
        }
    );
    assert_eq!(
        addresses.detect("Rosehip: status").unwrap(),
        AddressDetection::Addressed {
            matched_name_index: 1,
            utterance: "status".into(),
        }
    );
}

#[test]
fn address_values_have_one_bounded_canonical_encoding() {
    let addresses = AddressSet::new(&["Rosehip House", "Rosehip"]).unwrap();
    let encoded_addresses = encode_address_set(&addresses).unwrap();
    assert_eq!(decode_address_set(&encoded_addresses).unwrap(), addresses);

    for detection in [
        AddressDetection::NotAddressed,
        AddressDetection::Addressed {
            matched_name_index: 0,
            utterance: "status".into(),
        },
    ] {
        let encoded = encode_address_detection(&detection).unwrap();
        assert_eq!(decode_address_detection(&encoded).unwrap(), detection);
        let mut noncanonical = encoded;
        noncanonical.push(b' ');
        assert_eq!(
            decode_address_detection(&noncanonical),
            Err(AddressValueError::NonCanonical)
        );
    }
}

#[test]
fn unaddressed_or_substring_text_cannot_enter_the_model_path() {
    let addresses = AddressSet::new(&["Rosehip House", "Rose"]).unwrap();
    for text in [
        "what's the temperature upstairs?",
        "I walked past Rosehip House",
        "Rosemary, status",
    ] {
        assert_eq!(
            addresses.detect(text).unwrap(),
            AddressDetection::NotAddressed
        );
    }
}

#[test]
fn configuration_and_recognized_text_are_finite_and_unambiguous() {
    assert_eq!(AddressSet::new(&[]), Err(AddressConfigurationError::Empty));
    assert_eq!(
        AddressSet::new(&["Rose", "rose"]),
        Err(AddressConfigurationError::DuplicateName)
    );
    assert_eq!(
        AddressSet::new(&[" leading"]),
        Err(AddressConfigurationError::InvalidName)
    );
    let oversized_name = "x".repeat(MAX_ADDRESS_NAME_BYTES + 1);
    assert_eq!(
        AddressSet::new(&[&oversized_name]),
        Err(AddressConfigurationError::NameTooLarge)
    );
    let too_many = vec!["x"; MAX_ADDRESS_NAMES + 1];
    assert_eq!(
        AddressSet::new(&too_many),
        Err(AddressConfigurationError::TooManyNames)
    );
    let addresses = AddressSet::new(&["Rosehip House"]).unwrap();
    assert_eq!(
        addresses.detect(&"x".repeat(MAX_TEXT_BYTES as usize + 1)),
        Err(AddressDetectionError::RecognizedTextTooLarge)
    );
}

#[test]
fn canonical_address_detector_is_a_reusable_checked_form() {
    let mut startup = StartupCatalog::default();
    let mut profile = ProfileCatalog::default();
    install_text_catalogs(&mut startup, &mut profile).unwrap();
    let source = include_str!("../../../forms/addressed-utterance/main.conduit");
    let parsed = parse_syntax_document(source);
    assert!(parsed.diagnostics.is_empty());
    let checked = check_syntax_document(&parsed, &startup).unwrap();
    assert_eq!(checked.forms.len(), 1);
    let form = &checked.forms[0];
    assert_eq!(form.name, "addressed-utterance");
    assert_eq!(form.gears.len(), 1);
    let authored =
        expand_canonical_form_for_authoring(&checked, "addressed-utterance", &profile).unwrap();
    assert_eq!(authored.input_bindings.len(), 2);
    assert_eq!(authored.output_bindings.len(), 1);
    let gear = &authored.expanded.gears[0];
    assert_eq!(gear.kind_id.as_str(), ADDRESS_DETECT_KIND);
    assert_eq!(gear.inputs[1].value_kind.as_str(), ADDRESS_SET_VALUE_KIND);
    assert_eq!(
        gear.outputs[0].value_kind.as_str(),
        ADDRESS_DETECTION_VALUE_KIND
    );
}
