use super::*;

#[test]
fn semantic_faces_are_distinct_and_backend_free() {
    let encoded = alloc::format!("{:?}", sound_contracts_with_revisions());
    for forbidden in [
        "MIDI",
        "ALSA",
        "PipeWire",
        "OPL",
        "Create",
        "device-name",
        "default-output",
    ] {
        assert!(
            !encoded.contains(forbidden),
            "portable catalog contains {forbidden}"
        );
    }
    assert_ne!(music_play_contract().inputs, audio_play_contract().inputs);
    assert_eq!(
        MUSIC_PLAY_THROUGH_SYNTH.stages,
        [MUSIC_SYNTH_KIND, AUDIO_PLAY_KIND]
    );
}

#[test]
fn all_storage_and_pressure_are_finite() {
    for kind in [
        SOUND_TONE_PLAY_KIND,
        MUSIC_INPUT_KIND,
        MUSIC_PLAY_KIND,
        MUSIC_SYNTH_KIND,
        AUDIO_PLAY_KIND,
    ] {
        let semantics = stream_semantics(kind).unwrap();
        assert!(semantics.maximum_queue_items > 0);
        assert!(semantics.maximum_queue_bytes > 0);
        assert_eq!(
            semantics.pressure,
            PressureDisposition::WaitWithoutConsumption
        );
    }
}

#[cfg(feature = "form-catalog")]
#[test]
fn authored_synth_patch_has_exact_defaults_and_overrides() {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    crate::install_sound_catalogs(&mut startup, &mut profile).unwrap();
    let checked = conduit_form::parse(
        "form patch {\n synth: music/synth(maximum-voices = 12, oscillator = \"triangle\", filter-envelope-amount-q16 = -4096)\n}\n",
        &profile,
    )
    .unwrap();
    let configuration = &checked.gears[0].configuration;
    assert_eq!(configuration.len(), music_synth_configuration().len());
    assert_eq!(
        configuration
            .iter()
            .find(|entry| entry.key.as_str() == SYNTH_MAXIMUM_VOICES_KEY)
            .unwrap()
            .value,
        ConfigurationValue::U64(12)
    );
    assert_eq!(
        configuration
            .iter()
            .find(|entry| entry.key.as_str() == SYNTH_OSCILLATOR_KEY)
            .unwrap()
            .value,
        ConfigurationValue::Text("triangle".into())
    );
    assert_eq!(
        configuration
            .iter()
            .find(|entry| entry.key.as_str() == SYNTH_FILTER_ENVELOPE_KEY)
            .unwrap()
            .value,
        ConfigurationValue::I64(-4096)
    );
    assert_eq!(
        configuration
            .iter()
            .find(|entry| entry.key.as_str() == SYNTH_ATTACK_KEY)
            .unwrap()
            .value,
        ConfigurationValue::U64(10_000)
    );
}

#[cfg(feature = "form-catalog")]
#[test]
fn synth_playback_realization_is_an_ordinary_recursive_form() {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    crate::install_sound_catalogs(&mut startup, &mut profile).unwrap();
    for (kind, info) in [
        ("test/note-source", MUSIC_NOTE_INFO_ID),
        ("test/control-source", MUSIC_CONTROL_INFO_ID),
    ] {
        startup
            .insert(conduit_form::KindSignature {
                kind: kind.into(),
                startup_parameters: Vec::new(),
            })
            .unwrap();
        profile
            .insert(conduit_form::KindDefinition {
                kind_id: kind_id(kind),
                kind_contract_revision: KindContractRevision::from(alloc::format!("{kind}@1")),
                inputs: Vec::new(),
                outputs: vec![port("out", info, PortDirection::Output)],
                configuration: Vec::new(),
            })
            .unwrap();
    }
    let source = "form music/play-through-synth (\n > notes: music/note-event@1\n > controls: music/control-event@1\n) {\n synth: music/synth\n output: audio/play\n notes > synth.notes\n controls > synth.controls\n synth.audio > output.audio\n}\n\nform instrument-output {\n notes: test/note-source\n controls: test/control-source\n realization: music/play-through-synth\n notes > realization.notes\n controls > realization.controls\n}\n";
    let syntax = conduit_form::parse_syntax_document(source);
    let checked = conduit_form::check_syntax_document(&syntax, &startup).unwrap();
    let expanded =
        conduit_form::expand_canonical_form(&checked, "instrument-output", &profile).unwrap();
    assert_eq!(expanded.gears.len(), 4);
    assert!(expanded
        .gears
        .iter()
        .any(|gear| gear.kind_id.as_str() == MUSIC_SYNTH_KIND));
    assert!(expanded
        .gears
        .iter()
        .any(|gear| gear.kind_id.as_str() == AUDIO_PLAY_KIND));
    assert_eq!(expanded.connections.len(), 3);
    expanded.validate_expansion().unwrap();
}
