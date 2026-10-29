//! Storage choice cannot change configuration wire or canonical value meaning.
use conduit_core::{ConfigurationText, ConfigurationValue};
use serde::Serialize;

#[derive(Serialize)]
#[allow(
    dead_code,
    reason = "preserve the original four variant indices for wire comparison"
)]
enum OriginalValue {
    Bool(bool),
    U64(u64),
    I64(i64),
    Text(String),
}

#[test]
fn linked_text_keeps_its_allocation_and_the_original_json_and_postcard_wire() {
    const TEXT: &str = "é\0 linked \\\" program 🦀";
    let linked = ConfigurationText::from_static(TEXT);
    assert_eq!(linked.as_ptr(), TEXT.as_ptr());
    assert_eq!(linked.clone().as_ptr(), TEXT.as_ptr());
    let owned = ConfigurationText::from(TEXT.to_string());
    assert_eq!(linked, owned);
    assert_eq!(linked.cmp(&owned), std::cmp::Ordering::Equal);
    let old = OriginalValue::Text(TEXT.to_string());
    let value = ConfigurationValue::Text(linked);
    assert_eq!(
        serde_json::to_vec(&value).unwrap(),
        serde_json::to_vec(&old).unwrap()
    );
    assert_eq!(
        postcard::to_allocvec(&value).unwrap(),
        postcard::to_allocvec(&old).unwrap()
    );
    let received: ConfigurationValue =
        serde_json::from_slice(&serde_json::to_vec(&old).unwrap()).unwrap();
    assert_eq!(received, value);
    assert_eq!(ConfigurationValue::Text(owned), value);
}
