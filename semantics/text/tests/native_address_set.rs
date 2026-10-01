use conduit_form::rust_binding::NativeRustBinding;
use conduit_text::{AddressConfigurationError, AddressSet};

#[test]
fn address_set_round_trips_at_exact_name_and_count_bounds() {
    let name = "a".repeat(63);
    let names = (0..8)
        .map(|index| format!("{index}{name}"))
        .collect::<Vec<_>>();
    let names = names.iter().map(String::as_str).collect::<Vec<_>>();
    let addresses = AddressSet::new(&names).unwrap();
    let structured = addresses.clone().into_structured().unwrap();
    assert_eq!(AddressSet::from_structured(structured).unwrap(), addresses);

    assert_eq!(
        AddressSet::new(&[&"x".repeat(65)]),
        Err(AddressConfigurationError::NameTooLarge)
    );
}

#[test]
fn address_set_has_no_handwritten_semantic_record() {
    assert!(!include_str!("../src/addressed_utterance.rs").contains("pub struct AddressSet"));
}
