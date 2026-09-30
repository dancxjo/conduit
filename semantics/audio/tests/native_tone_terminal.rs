use conduit_audio::{audio_tone_terminal_kind_id, AudioToneTerminal, Gate, ModulationDestination};
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

#[test]
fn pcm_unit_variants_have_native_semantic_identity_and_exact_round_trips() {
    use conduit_audio::{PcmChannelLayout, PcmSampleRepresentation};

    for representation in [
        PcmSampleRepresentation::Signed16LittleEndian,
        PcmSampleRepresentation::Signed24LittleEndian,
        PcmSampleRepresentation::Float32LittleEndian,
    ] {
        let structured = representation.into_structured().unwrap();
        assert_eq!(
            PcmSampleRepresentation::from_structured(structured).unwrap(),
            representation
        );
    }
    for layout in [PcmChannelLayout::Mono, PcmChannelLayout::StereoLeftRight] {
        let structured = layout.into_structured().unwrap();
        assert_eq!(
            PcmChannelLayout::from_structured(structured).unwrap(),
            layout
        );
    }
}

#[test]
fn musical_unit_variants_have_native_semantic_identity_and_exact_round_trips() {
    for gate in [Gate::On, Gate::Off] {
        let structured = gate.into_structured().unwrap();
        assert_eq!(Gate::from_structured(structured).unwrap(), gate);
    }
    for destination in [
        ModulationDestination::Pitch,
        ModulationDestination::FilterCutoff,
        ModulationDestination::Amplitude,
    ] {
        let structured = destination.into_structured().unwrap();
        assert_eq!(
            ModulationDestination::from_structured(structured).unwrap(),
            destination
        );
    }
}
