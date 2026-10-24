#![cfg(feature = "semantic-bindings")]
use conduit_plot::*;
use std::collections::BTreeSet;
#[path = "common/native_law_program.rs"]
mod native_law_program;
use native_law_program::*;

#[test]
fn all_four_g2_program_native_owners_have_complete_replayable_custody() {
    let original = conduit_speech::authoring::checked_document_with_law_custody().unwrap();
    let speech_metadata = conduit_speech::authoring::checked_types().unwrap();
    let audio_metadata = conduit_audio::authoring::checked_types().unwrap();
    assert_eq!(original.native_types, speech_metadata);
    let mut all = BTreeSet::new();
    let mut programs = Vec::new();
    for name in ["advance", "phases", "curve", "energy"] {
        let name = format!("speech/phone-gesture-{name}");
        let encoded = program(&original, &name);
        let value = PortableExpressionProgram::from_canonical_hex(&encoded).unwrap();
        types(&value.input_type, &mut all);
        types(&value.output_type, &mut all);
        nodes(&value.root, &mut all);
        programs.push((name, encoded));
    }
    let mut speech_names = BTreeSet::new();
    let mut audio_names = BTreeSet::new();
    for ty in all {
        if let Some(owner) = speech_metadata.iter().find(|owner| owner.value_type == ty) {
            speech_names.insert(owner.name.as_str());
        } else if let Some(owner) = audio_metadata.iter().find(|owner| owner.value_type == ty) {
            audio_names.insert(owner.name.as_str());
        } else {
            assert_builtin_structure(&ty);
        }
    }
    let speech = conduit_speech::authoring::checked_document_with_law_custody_for(
        &speech_names.iter().copied().collect::<Vec<_>>(),
    )
    .unwrap();
    let audio = conduit_audio::authoring::checked_document_with_law_custody_for(
        &audio_names.iter().copied().collect::<Vec<_>>(),
    )
    .unwrap();
    assert_eq!(speech.source_document_id, original.source_document_id);
    assert_eq!(speech.native_types, original.native_types);
    assert_eq!(audio.native_types, audio_metadata);
    for (name, encoded) in programs {
        assert_eq!(
            program(&speech, &name),
            encoded,
            "original canonical program unchanged"
        );
    }
    for (document, expected, names) in [
        (&speech, &speech_metadata, &speech_names),
        (&audio, &audio_metadata, &audio_names),
    ] {
        for name in names {
            let expected = expected.iter().find(|owner| owner.name == *name).unwrap();
            let witness = document.native_law_witness(name).unwrap();
            assert_eq!(witness.checked_type(), expected);
            let anchors = witness.family_anchors().unwrap();
            let material = witness.material().unwrap();
            let replay = NativeLawWitness::verify_with_context(
                &material,
                &expected.value_type,
                &NativeLawContext { families: &anchors },
            )
            .unwrap();
            assert_eq!(replay.checked_type(), expected);
        }
    }
    assert!(speech_names.contains("SpeechPhoneGestureTiming"));
    assert!(audio_names.contains("AudioQuantityTrajectory"));
}

#[test]
fn requested_roots_are_transactional_and_cannot_upgrade_forged_metadata() {
    let mut catalog = StartupCatalog::new();
    let before = catalog.clone();
    assert!(conduit_speech::authoring::install_law_custody_for(
        &mut catalog,
        &["SpeechPhoneGestureTiming", "Absent"]
    )
    .is_err());
    assert_eq!(catalog, before);
    let doc = conduit_speech::authoring::checked_document_with_law_custody_for(&[
        "SpeechPhoneGestureTiming",
    ])
    .unwrap();
    let mut forged = doc
        .native_law_witness("SpeechPhoneGestureTiming")
        .unwrap()
        .checked_type()
        .clone();
    assert!(!forged.invariants.is_empty());
    forged.invariants.clear();
    catalog
        .insert_checked_native_type("SpeechPhoneGestureTiming", &forged)
        .unwrap();
    let before = catalog.clone();
    assert!(conduit_speech::authoring::install_law_custody_for(
        &mut catalog,
        &["SpeechPhoneGestureTiming"]
    )
    .is_err());
    assert_eq!(catalog, before);
}

#[test]
fn witnessed_import_preserves_legacy_generated_type_identity() {
    let mut legacy = StartupCatalog::new();
    conduit_speech::authoring::install(&mut legacy).unwrap();
    let mut witnessed = StartupCatalog::new();
    conduit_speech::authoring::install_law_custody_for(
        &mut witnessed,
        &["SpeechPhoneGestureTiming"],
    )
    .unwrap();
    let source = parse_syntax_document("type Imported = {\n    value: SpeechPhoneGestureTiming\n}");
    let old = check_syntax_document(&source, &legacy).unwrap();
    let new = check_syntax_document(&source, &witnessed).unwrap();
    assert_eq!(old.native_types, new.native_types);
    assert!(new.native_law_witness("Imported").is_some());
    assert!(old.native_law_witness("Imported").is_none());
}
