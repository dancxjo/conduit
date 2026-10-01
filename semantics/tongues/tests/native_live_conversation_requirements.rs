use conduit_form::rust_binding::NativeRustBinding;
use conduit_tongues::LiveConversationSpeechRequirements;

#[test]
fn live_conversation_requirements_are_exact_native_finite_values() {
    let value = LiveConversationSpeechRequirements::new(u32::MAX, u16::MAX, 0, 1).unwrap();
    assert_eq!(value.recognition_audio_bytes(), u32::MAX);
    assert_eq!(value.recognized_text_bytes(), u16::MAX);
    assert_eq!(value.speakable_segment_bytes(), 0);
    assert_eq!(value.synthesized_pcm_bytes(), 1);

    let structured = value.into_structured().unwrap();
    assert_eq!(
        LiveConversationSpeechRequirements::from_structured(structured),
        Ok(value)
    );
}
