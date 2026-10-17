#![cfg(feature = "semantic-bindings")]
#[path = "common/committed_carrier/commitment.rs"]
mod commitment;
#[path = "common/committed_carrier/constructors.rs"]
mod constructors;
#[path = "common/committed_carrier/inspection.rs"]
mod inspection;
#[path = "common/committed_carrier/material.rs"]
mod material;
#[path = "common/committed_carrier/producer.rs"]
mod producer;
#[path = "common/committed_carrier/transport.rs"]
mod transport;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::semantic::*;

#[test]
fn one_committed_multiword_carrier_survives_source_bytes_and_independent_inspection() {
    let bytes = producer::serialize();
    let inspected = inspection::inspect_original(&bytes, inspection::identity(&bytes)).unwrap();
    assert_eq!(inspected.original, bytes);
    assert_eq!(inspected.text, "would you weigh");
    assert_eq!(inspected.committed_words, 3);
    assert_eq!(inspected.intent, material::intent());
    assert_eq!(inspected.phonemes, material::phonemes());
    assert_eq!(inspected.phones, material::phones());
    assert_eq!(inspected.alignment, material::alignment());
    assert_eq!(inspected.word_phonemes, material::word_phonemes());
    assert_eq!(inspected.syllables, material::syllables());
    assert_eq!(inspected.boundaries, material::boundaries());
    assert_eq!(inspected.syllables.len(), 3);
    assert_eq!(inspected.boundaries.len(), 8);
    let mut states = Vec::new();
    for event in inspected.intent.events().as_slice().iter().take(6) {
        let SpeechUtteranceIntentEvent::Segment(value) = event else {
            panic!("segment")
        };
        states.push(value.prosody().duration().clone().encode().unwrap());
    }
    for (index, state) in states.iter().enumerate() {
        assert!(!states[..index].contains(state));
    }
    let SpeechRealizationCorrespondence::Aligned(coalesced) =
        &inspected.alignment.correspondences()[2]
    else {
        panic!("coalescence")
    };
    assert_eq!(coalesced.phonemes().len(), 2);
    assert_eq!(coalesced.phones().len(), 1);
    let SpeechRealizationCorrespondence::Aligned(diphthong) =
        &inspected.alignment.correspondences()[5]
    else {
        panic!("offglide")
    };
    assert_eq!(diphthong.phonemes().len(), 1);
    assert_eq!(diphthong.phones().len(), 2);
}

#[test]
fn stale_commitment_missing_foreign_and_duplicate_segments_refuse_after_decode() {
    let bytes = producer::serialize();
    let original: Vec<Vec<u8>> = postcard::from_bytes(&bytes).unwrap();
    let mut cases = Vec::new();
    let mut stale = original.clone();
    let request = conduit_language::LanguageParserBeamCommitRequest::decode(&stale[12]).unwrap();
    let wrong = conduit_language::LanguageParserBasis::new(
        request.beam().basis().analysis_revision().clone(),
        conduit_language::LanguageTextRevisionId::new("source/stale".into()).unwrap(),
        request.beam().basis().text().clone(),
    )
    .unwrap();
    let frontier = conduit_language::LanguageParserRawFrontier::new(wrong, 0, 3).unwrap();
    stale[12] = conduit_language::LanguageParserBeamCommitRequest::new(
        request.beam().clone(),
        false,
        frontier,
        false,
        3,
    )
    .unwrap()
    .encode()
    .unwrap();
    cases.push(stale);
    let mut missing = original.clone();
    let phones = SpeechPhoneSequence::decode(&missing[4]).unwrap();
    missing[4] = SpeechPhoneSequence::new(
        phones.basis().clone(),
        material::seq(phones.tokens().as_slice()[1..].iter().cloned()),
    )
    .unwrap()
    .encode()
    .unwrap();
    cases.push(missing);
    let mut duplicate = original.clone();
    let intent = SpeechUtteranceIntent::decode(&duplicate[2]).unwrap();
    duplicate[2] = SpeechUtteranceIntent::new(
        material::seq(
            intent
                .events()
                .as_slice()
                .iter()
                .cloned()
                .chain([intent.events()[0].clone()]),
        ),
        intent.inventory_id().clone(),
        intent.language().clone(),
        intent.provenance().clone(),
        intent.revision_id().clone(),
        intent.utterance_id().clone(),
    )
    .unwrap()
    .encode()
    .unwrap();
    cases.push(duplicate);
    let mut foreign = original.clone();
    let phones = SpeechPhoneSequence::decode(&foreign[4]).unwrap();
    let b = phones.basis();
    let foreign_basis = SpeechTokenSequenceBasis::new(
        b.inventory_id().clone(),
        b.language().clone(),
        b.revision_id().clone(),
        SpeechSegmentSequenceId::new("foreign".into()).unwrap(),
        b.utterance_id().clone(),
    )
    .unwrap();
    foreign[4] = SpeechPhoneSequence::new(foreign_basis, phones.tokens().clone())
        .unwrap()
        .encode()
        .unwrap();
    cases.push(foreign);
    let mut foreign_variety = original.clone();
    let mut syllables: Vec<Vec<u8>> = postcard::from_bytes(&foreign_variety[10]).unwrap();
    let s = SpeechSyllableSource::decode(&syllables[0]).unwrap();
    syllables[0] = SpeechSyllableSource::new(
        s.ipa().clone(),
        s.ipa_phone_indices().clone(),
        s.syllable().clone(),
        conduit_language::LanguageVariety::new(
            VarietyId::new("foreign/variety".into()).unwrap(),
            s.variety().language().clone(),
        )
        .unwrap(),
        s.word().clone(),
    )
    .unwrap()
    .encode()
    .unwrap();
    foreign_variety[10] = postcard::to_allocvec(&syllables).unwrap();
    cases.push(foreign_variety);
    let mut excess = original.clone();
    excess.push(vec![]);
    cases.push(excess);
    let mut changed_source = original.clone();
    changed_source[14].push(b' ');
    cases.push(changed_source);
    for candidate in cases {
        assert!(inspection::inspect(&postcard::to_allocvec(&candidate).unwrap()).is_err());
    }
}

#[test]
fn fresh_process_inspects_only_original_serialized_material() {
    let bytes = producer::serialize();
    let expected = inspection::identity(&bytes);
    let path = std::env::temp_dir().join(format!("conduit-carrier-{}.native", std::process::id()));
    std::fs::write(&path, &bytes).unwrap();
    let hex = expected
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "independent_process_inspector", "--nocapture"])
        .env("CONDUIT_CARRIER_INSPECTION_FILE", &path)
        .env("CONDUIT_CARRIER_INSPECTION_ID", hex)
        .output()
        .unwrap();
    std::fs::remove_file(&path).unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(String::from_utf8_lossy(&result.stdout).contains(
        "3 committed words; 7 intended phonemes; 7 target phones; 3 syllables; 8 boundaries"
    ));
    for identity in [
        "contrast/d",
        "contrast/j",
        "target/affricate",
        "contrast/diphthong",
        "target/glide",
        "speech/1",
        "would you weigh",
    ] {
        assert!(String::from_utf8_lossy(&result.stdout).contains(identity));
    }
    if let Some(path) = std::env::var_os("CONDUIT_COMMITTED_CARRIER_OUTPUT") {
        let path = std::path::PathBuf::from(path);
        std::fs::write(&path, &bytes).unwrap();
        let view = inspection::inspect_original(&bytes, expected).unwrap();
        let receipt = serde_json::json!({
            "proof_class":"local checked Source, Native re-admission, kernel semantic commitment and fresh-process inspection",
            "accepted_dev":false,"identity_domain":inspection::IDENTITY_DOMAIN,
            "identity":expected.iter().map(|b|format!("{b:02x}")).collect::<String>(),"bytes":bytes.len(),
            "text":view.text,"utterance":view.intent.utterance_id().get(),"speech_revision":view.intent.revision_id().get(),
            "inventory":view.intent.inventory_id().get(),"inventory_revision":view.alignment.inventory_basis().revision().get(),
            "variety":view.alignment.inventory_basis().variety().identity().get(),"committed_words":view.committed_words,
            "intended_phonemes":view.phonemes.tokens().len(),"target_phones":view.phones.tokens().len(),
            "syllables":view.syllables.len(),"boundaries":view.boundaries.len(),"specification_states":6,
            "acoustic_or_neural_integration":false,"upstream_pr":"https://github.com/dancxjo/conduit/pull/5925"
        });
        std::fs::write(
            path.with_extension("json"),
            serde_json::to_vec_pretty(&receipt).unwrap(),
        )
        .unwrap();
    }
    let mut changed: Vec<Vec<u8>> = postcard::from_bytes(&bytes).unwrap();
    let mut syllables: Vec<Vec<u8>> = postcard::from_bytes(&changed[10]).unwrap();
    let s = SpeechSyllableSource::decode(&syllables[2]).unwrap();
    let old = s.syllable();
    let new = SpeechPlannedSyllableIntent::new(
        old.basis().clone(),
        old.identity().clone(),
        *old.nucleus_index(),
        old.phone_positions().clone(),
        old.phones().clone(),
        old.provenance().clone(),
        old.span().clone(),
        StressSpecification::known(SpeechStress::Secondary).unwrap(),
    )
    .unwrap();
    syllables[2] = SpeechSyllableSource::new(
        s.ipa().clone(),
        s.ipa_phone_indices().clone(),
        new,
        s.variety().clone(),
        s.word().clone(),
    )
    .unwrap()
    .encode()
    .unwrap();
    changed[10] = postcard::to_allocvec(&syllables).unwrap();
    assert!(
        inspection::inspect_original(&postcard::to_allocvec(&changed).unwrap(), expected).is_err()
    );
}

#[test]
fn independent_process_inspector() {
    let Some(path) = std::env::var_os("CONDUIT_CARRIER_INSPECTION_FILE") else {
        return;
    };
    let hex = std::env::var("CONDUIT_CARRIER_INSPECTION_ID").unwrap();
    assert_eq!(hex.len(), 64);
    let mut expected = [0; 32];
    for (n, part) in expected.iter_mut().enumerate() {
        *part = u8::from_str_radix(&hex[n * 2..n * 2 + 2], 16).unwrap();
    }
    let result = inspection::inspect_original(&std::fs::read(path).unwrap(), expected).unwrap();
    let pairs = result
        .alignment
        .correspondences()
        .as_slice()
        .iter()
        .map(|group| {
            let SpeechRealizationCorrespondence::Aligned(group) = group else {
                panic!("explicit relation")
            };
            let phonemes = group
                .phonemes()
                .as_slice()
                .iter()
                .map(
                    |r| match result.phonemes.tokens()[*r.ordinal() as usize].phoneme() {
                        PhonemeSpecification::Known(id) => id.get().to_owned(),
                        _ => panic!("known member"),
                    },
                )
                .collect::<Vec<_>>();
            let phones = group
                .phones()
                .as_slice()
                .iter()
                .map(
                    |r| match result.phones.tokens()[*r.ordinal() as usize].phone() {
                        PhoneSpecification::Known(id) => id.get().to_owned(),
                        _ => panic!("known member"),
                    },
                )
                .collect::<Vec<_>>();
            (phonemes, phones)
        })
        .collect::<Vec<_>>();
    println!(
        "original text: {}; utterance: {}; revision: {}; inventory: {}; correspondences: {:?}",
        result.text,
        result.intent.utterance_id().get(),
        result.intent.revision_id().get(),
        result.intent.inventory_id().get(),
        pairs
    );
    println!(
        "{} committed words; {} intended phonemes; {} target phones; {} syllables; {} boundaries",
        result.committed_words,
        result.phonemes.tokens().len(),
        result.phones.tokens().len(),
        result.syllables.len(),
        result.boundaries.len()
    );
}

#[test]
fn checked_intent_source_reconstruction() {
    let original = material::intent();
    assert_eq!(transport::execute(&original), original);
}
