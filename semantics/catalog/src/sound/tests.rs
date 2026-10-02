use super::*;

#[test]
fn push_to_talk_is_one_host_neutral_bounded_pcm_flow() {
    let contract = audio_capture_push_to_talk_contract();
    assert_eq!(contract.kind_id.as_str(), AUDIO_CAPTURE_PUSH_TO_TALK_KIND);
    assert!(contract.inputs.is_empty());
    assert_eq!(contract.outputs.len(), 1);
    assert_eq!(contract.outputs[0].port_id.as_str(), "audio");
    assert_eq!(contract.outputs[0].value_kind.as_str(), AUDIO_PCM_INFO_ID);
    assert_eq!(
        contract.outputs[0].temporal,
        PortTemporal::Flow { closes: true }
    );
    assert_eq!(contract.configuration.len(), 1);
    assert_eq!(
        contract.configuration[0].key,
        AUDIO_CAPTURE_MAXIMUM_TURN_MILLIS_KEY
    );
    for forbidden in ["browser", "web", "device", "permission", "microphone api"] {
        assert!(!contract.summary.to_ascii_lowercase().contains(forbidden));
    }
}

#[test]
fn audio_tone_is_exactly_typed_bounded_and_cancellable() {
    let contract = audio_tone_semantic_contract();
    assert_eq!(
        contract.inputs[0].value_kind.as_str(),
        conduit_core::FREQUENCY_INFO_ID
    );
    assert_eq!(contract.inputs[0].temporal, PortTemporal::Current);
    assert_eq!(contract.limits.max_queue_items, 1);
    assert_eq!(contract.limits.max_queue_bytes, AUDIO_TONE_PCM_BLOCK_BYTES);
    assert_eq!(contract.outputs[0].value_kind.as_str(), AUDIO_PCM_INFO_ID);
    assert_eq!(
        contract.outputs[0].abnormal_kind.as_ref().unwrap().as_str(),
        conduit_audio::audio_tone_terminal_kind_id().as_str()
    );
    let terminal = contract.terminal_transductions().next().unwrap();
    assert_eq!(terminal.input_port_id, conduit_core::port_id("frequency"));
    assert_eq!(terminal.output_port_id, conduit_core::port_id("audio"));
    assert_eq!(
        terminal.normal_close,
        conduit_core::NormalCloseTransduction::NotAccepted
    );
    assert_eq!(
        terminal.abnormal,
        conduit_core::AbnormalTerminalTransduction::NotAccepted
    );
    assert_eq!(
        terminal.cancellation,
        conduit_core::CancellationTransduction::Request {
            disposition_kind: conduit_audio::audio_tone_terminal_kind_id(),
        }
    );
    contract.validate().unwrap();
}

#[test]
fn audio_gain_admits_one_whole_pcm_block_atomically() {
    let contract = audio_gain_contract();
    assert_eq!(contract.limits.max_queue_items, 1);
    assert_eq!(
        contract.limits.max_queue_bytes,
        AUDIO_CONTINUOUS_TONE_PCM_BLOCK_BYTES
    );
}

#[test]
fn semantic_fronts_are_distinct_and_backend_free() {
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

#[cfg(feature = "plot-catalog")]
#[test]
fn authored_synth_patch_has_exact_defaults_and_overrides() {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    crate::install_sound_catalogs(&mut startup, &mut profile).unwrap();
    let checked = conduit_plot::parse(
        "plot patch {\n synth: music/synth(maximum-voices = 12, oscillator = \"triangle\", filter-envelope-amount-q16 = -4096)\n}\n",
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

#[cfg(feature = "plot-catalog")]
#[test]
fn synth_playback_realization_is_an_ordinary_recursive_plot() {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    crate::install_sound_catalogs(&mut startup, &mut profile).unwrap();
    for (kind, info) in [
        ("test/note-source", MUSIC_NOTE_INFO_ID),
        ("test/control-source", MUSIC_CONTROL_INFO_ID),
    ] {
        startup
            .insert(conduit_plot::KindSignature {
                kind: kind.into(),
                startup_parameters: Vec::new(),
            })
            .unwrap();
        profile
            .insert(conduit_plot::KindProjection {
                kind_id: kind_id(kind),
                kind_contract_revision: KindIdentity::from(alloc::format!("{kind}@1")),
                inputs: Vec::new(),
                outputs: vec![port("out", info, PortDirection::Output)],
                configuration: Default::default(),
            })
            .unwrap();
    }
    let source = "plot music/play-through-synth (\n >> notes: music/note-event@1\n >> controls: music/control-event@1\n) {\n synth: music/synth\n output: audio/play\n notes >> synth.notes\n controls >> synth.controls\n synth.audio >> output.audio\n}\n\nplot instrument-output {\n notes: test/note-source\n controls: test/control-source\n realization: music/play-through-synth\n notes >> realization.notes\n controls >> realization.controls\n}\n";
    let syntax = conduit_plot::parse_syntax_document(source);
    let checked = conduit_plot::check_syntax_document(&syntax, &startup).unwrap();
    let expanded =
        conduit_plot::expand_canonical_plot(&checked, "instrument-output", &profile).unwrap();
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
