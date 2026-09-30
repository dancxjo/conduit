use conduit_audio::{audio_tone_terminal_kind_id, AudioToneTerminal};
use conduit_core::StructuredInfoTypeShape;
use conduit_form::rust_binding::NativeRustBinding;

#[test]
fn native_tone_terminal_owns_semantic_identity_while_wire_codec_stays_explicit() {
    let semantic = AudioToneTerminal::semantic_type().unwrap();
    let StructuredInfoTypeShape::Variant { schema, .. } = semantic.shape() else {
        panic!("audio tone terminal must remain a semantic variant")
    };
    assert_eq!(schema, &audio_tone_terminal_kind_id());

    let structured = AudioToneTerminal::Cancelled.into_structured().unwrap();
    assert_eq!(
        AudioToneTerminal::from_structured(structured).unwrap(),
        AudioToneTerminal::Cancelled
    );

    assert_eq!(AudioToneTerminal::Cancelled.encode(), [0]);
    assert_eq!(
        AudioToneTerminal::decode(&[0]).unwrap(),
        AudioToneTerminal::Cancelled
    );
}
