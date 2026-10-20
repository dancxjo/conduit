//! Opaque checked argument bytes are not an unsupported text footer.
use super::*;

#[test]
fn checked_binary_argument_keeps_exact_bytes_without_text_preview() {
    let contract = conduit_core::CheckedValueContract {
        value_kind: conduit_core::kind_id("test/opaque-bytes"),
        maximum_bytes: 2,
        constraints: vec![],
    };
    let bytes = vec![0xff, 0xfe];
    contract.validate(&bytes).unwrap();
    let input = InputState {
        action: Some(0),
        values: vec![Some(bytes.clone())],
        ..Default::default()
    };
    let chrome = super::super::chrome::Chrome::prepare(
        LayoutRect {
            x: 0,
            y: 0,
            width: 640,
            height: 480,
        },
        100,
    )
    .unwrap();
    assert_eq!(
        input
            .preview(
                Some(FaceControl {
                    action: 0,
                    argument: Some(0)
                }),
                &chrome,
                100
            )
            .unwrap(),
        None
    );
    assert_eq!(input.values[0].as_ref().unwrap(), &bytes);
}
