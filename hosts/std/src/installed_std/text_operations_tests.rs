use super::text_operations::{prefix_utf8, uppercase_utf8};

#[test]
fn text_transforms_reject_invalid_utf8_and_combined_overflow() {
    let mut output = Vec::with_capacity(conduit_std_catalog::MAX_TEXT_BYTES as usize);
    assert!(uppercase_utf8(&[0xff], &mut output)
        .unwrap_err()
        .contains("not valid UTF-8"));
    assert!(prefix_utf8("prefix", &[0xff], &mut output)
        .unwrap_err()
        .contains("not valid UTF-8"));
    let prefix = "x".repeat(conduit_std_catalog::MAX_TEXT_BYTES as usize);
    assert!(prefix_utf8(&prefix, b"y", &mut output)
        .unwrap_err()
        .contains("output exceeds"));
    assert!(output.is_empty());
}

#[test]
fn text_transforms_reuse_the_preallocated_adapter_buffer() {
    let mut output = Vec::with_capacity(conduit_std_catalog::MAX_TEXT_BYTES as usize);
    let capacity = output.capacity();
    uppercase_utf8("Straße".as_bytes(), &mut output).unwrap();
    assert_eq!(output, "STRASSE".as_bytes());
    prefix_utf8("Welcome", b"Travis", &mut output).unwrap();
    assert_eq!(output, b"WelcomeTravis");
    assert_eq!(output.capacity(), capacity);
}
