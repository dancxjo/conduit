#![cfg(all(feature = "semantic-bindings", feature = "kernel"))]
#[path = "common/vocative_language.rs"]
#[allow(dead_code)]
mod language;
use conduit_language::{lexical::prepare_lexical_tape, *};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_speech::committed_token_role;
use conduit_speech::{
    phonemic_pronunciation::*, phonemic_pronunciation_intent::*, plan_coverage::*, semantic::*,
};
fn bounded_read(name: &str) -> Vec<u8> {
    let path = std::env::var(name).unwrap();
    assert!(std::fs::metadata(&path).unwrap().len() <= 32 * 1024 * 1024);
    std::fs::read(path).unwrap()
}
fn native<T: NativeRustBinding>(v: &serde_json::Value) -> T {
    T::decode(&serde_json::from_value::<Vec<u8>>(v.clone()).unwrap()).unwrap()
}
#[test]
#[ignore = "requires original retained four-revision Language commitments"]
fn actual_committed_complete_greeting_preserves_phonemic_pronunciation_custody() {
    let material: serde_json::Value =
        serde_json::from_slice(&bounded_read("CONDUIT_WORD_STREAM_COMMITTED_ROLES")).unwrap();
    let events = String::from_utf8(bounded_read("CONDUIT_WORD_STREAM_EVENTS")).unwrap();
    let snapshot = events
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .find(|row| {
            row["event"] == "snapshot" && row["source_sequence"] == 3 && row["committed"] == 3
        })
        .unwrap();
    let runtime: LanguageParserJointRuntimeBeam = native(&snapshot["beam_bytes"]);
    let beam = runtime.beam();
    assert_eq!(
        beam.lexical().tape().source().material().text(),
        "Hello, Travis."
    );
    let mut previous = None;
    for bytes in material["source_revision_history_bytes"]
        .as_array()
        .unwrap()
    {
        let revision: LanguageTextRevision = native(bytes);
        previous = Some(
            prepare_lexical_tape(
                &revision,
                beam.lexical().tape().profile(),
                previous.as_ref(),
            )
            .unwrap(),
        );
    }
    let lexical = previous.unwrap();
    assert_eq!(lexical.tape(), beam.lexical().tape());
    let state = beam.candidate0().parser().state();
    let token = |ordinal: usize| {
        LanguageAnalysisTokenRef::new(
            beam.basis().analysis_revision().clone(),
            lexical.tape().tokens()[ordinal].identity().clone(),
        )
        .unwrap()
    };
    let arcs = (0..4)
        .map(|dependent| {
            let relation = match dependent {
                0 => state.relation0(),
                1 => state.relation1(),
                2 => state.relation2(),
                _ => state.relation3(),
            };
            let head = state.heads()[dependent];
            let governor = if head == 4 {
                LanguageDependencyHead::root()
            } else {
                let reference = token(head as usize);
                LanguageDependencyHead::token(
                    reference.revision().clone(),
                    reference.token().clone(),
                )
                .unwrap()
            };
            let subtype = (!relation.subtype().get().is_empty())
                .then(|| LanguageDependencySubtype::new(relation.subtype().get().clone()).unwrap());
            LanguageDependencyArc::new(
                token(dependent),
                governor,
                LanguageDependencyRelation::new(*relation.base(), subtype).unwrap(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let choices = beam
        .candidate0()
        .choices()
        .iter()
        .map(|choice| *choice as usize)
        .collect::<Vec<_>>();
    let case = language::admitted_graph_with_choices(
        lexical,
        beam.basis().analysis_revision().clone(),
        arcs,
        2,
        &choices,
    );
    let commitments = material["commitments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|word| {
            native::<LanguageParserCommittedDependencyAdmission>(
                &word["committed_dependency_admission_bytes"],
            )
        })
        .collect::<Vec<_>>();
    let roles = commitments
        .iter()
        .map(|commitment| {
            committed_token_role::prepare_committed_token_role(&case.lexical, commitment).unwrap()
        })
        .collect::<Vec<_>>();
    assert!(roles
        .iter()
        .all(|role| matches!(role.role().result().role(), SpeechTextTokenRole::Spoken)));
    let unit = |ipa: &str, stress| {
        SpeechPhonemicPronunciationPhoneme::new(
            PhonemeId::new(format!("phoneme/{ipa}")).unwrap(),
            stress,
        )
        .unwrap()
    };
    let row = |lemma, pos, units: &[(&str, SpeechStress)]| {
        SpeechPhonemicPronunciationRow::new(
            language::candidate(lemma, pos),
            BoundedSequence::try_from_iter(units.iter().map(|(ipa, stress)| unit(ipa, *stress)))
                .unwrap(),
        )
        .unwrap()
    };
    let profile = SpeechPhonemicPronunciationProfile::new(
        "reviewed/greeting/phonemic-v1".into(),
        case.inventory.language().clone(),
        language::speech_provenance(),
        BoundedSequence::try_from_iter([
            row(
                "Hello",
                LanguageLexicalPos::Interjection,
                &[
                    ("h", SpeechStress::Unstressed),
                    ("ə", SpeechStress::Unstressed),
                    ("l", SpeechStress::Primary),
                    ("oʊ", SpeechStress::Primary),
                ],
            ),
            row(
                "Travis",
                LanguageLexicalPos::ProperNoun,
                &[
                    ("t", SpeechStress::Primary),
                    ("ɹ", SpeechStress::Primary),
                    ("æ", SpeechStress::Primary),
                    ("v", SpeechStress::Unstressed),
                    ("ɪ", SpeechStress::Unstressed),
                    ("s", SpeechStress::Unstressed),
                ],
            ),
        ])
        .unwrap(),
    )
    .unwrap();
    let prepared = case
        .selections
        .iter()
        .map(|selection| prepare_phonemic_pronunciation(selection, &profile).unwrap())
        .collect::<Vec<_>>();
    let inventory = SpeechInventoryId::new("reviewed/greeting/common-ipa-v1".into()).unwrap();
    let revision = SpeechSegmentRevisionId::new("phonemic/0".into()).unwrap();
    let utterance = SpeechUtteranceId::new("phonemic/greeting".into()).unwrap();
    let origin = |sequence: String, ordinal| {
        LanguageSpeechTokenRef::new(
            inventory.clone(),
            profile.language().clone(),
            ordinal,
            revision.clone(),
            SpeechSegmentSequenceId::new(sequence).unwrap(),
            utterance.clone(),
        )
        .unwrap()
    };
    let prosody = SpeechSegmentProsodyIntent::new(
        SpeechDurationSpecification::known(5, 1).unwrap(),
        SpeechCycleSpecification::known(200, 1).unwrap(),
        SpeechIntensitySpecification::known(1, 1).unwrap(),
    )
    .unwrap();
    let words = prepared
        .iter()
        .enumerate()
        .map(|(i, p)| {
            prepare_phonemic_pronunciation_intent(
                p,
                &origin(format!("word/{i}"), 0),
                &prosody,
                profile.provenance(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let refs = words
        .iter()
        .map(PreparedSpeechPronunciationWord::Phoneme)
        .collect::<Vec<_>>();
    let order =
        prepare_complete_spoken_order(&case.lexical, &case.participation, &case.spoken_ordinals)
            .unwrap();
    assert!(matches!(
        prepare_complete_pronunciation_layout(&order, &refs[..1]),
        Err(SpeechPlanCoverageRefusal::MissingWord)
    ));
    let swapped = [refs[1], refs[0]];
    assert!(matches!(
        prepare_complete_pronunciation_layout(&order, &swapped),
        Err(SpeechPlanCoverageRefusal::ForeignWord { word: 0 })
    ));
    let layout = prepare_complete_pronunciation_layout(&order, &refs).unwrap();
    let mut events = Vec::new();
    let mut witnesses = Vec::new();
    for (position, word) in words.iter().enumerate() {
        for event in word.intent().events().iter() {
            let SpeechUtteranceIntentEvent::Segment(original) = event else {
                panic!("phonemic segment")
            };
            let occurrence = origin("common/target-phones".into(), events.len() as u32);
            let event = SpeechUtteranceIntentEvent::segment(
                occurrence.clone(),
                original.phone().clone(),
                original.phoneme().clone(),
                original.prosody().clone(),
                original.provenance().clone(),
                original.sources().clone(),
                original.stress().clone(),
                original.word_position().clone(),
            )
            .unwrap();
            let SpeechUtteranceIntentEvent::Segment(current) = &event else {
                unreachable!()
            };
            witnesses.push(
                SpeechPhoneCompositionWitness::new(
                    admit_planned_segment_material(current).unwrap(),
                    events.len() as u32,
                    layout.clone(),
                    admit_planned_segment_material(original).unwrap(),
                    SpeechOccurrenceMembership::new(
                        inventory.clone(),
                        profile.language().clone(),
                        occurrence,
                        revision.clone(),
                        utterance.clone(),
                    )
                    .unwrap(),
                    position as u64,
                )
                .unwrap(),
            );
            events.push(event);
        }
    }
    let composite = SpeechUtteranceIntent::new(
        BoundedSequence::try_from_iter(events).unwrap(),
        inventory,
        profile.language().clone(),
        profile.provenance().clone(),
        revision,
        utterance,
    )
    .unwrap();
    let coverage =
        prepare_pronunciation_plan_coverage(&order, &refs, &composite, &witnesses).unwrap();
    assert_eq!(coverage.phone_events().len(), 10);
    assert!(core::ptr::eq(coverage.intent(), &composite));
    assert_eq!(coverage.layout(), &layout);
    for (actual, original) in coverage.words().iter().zip(&words) {
        let PreparedSpeechPronunciationWord::Phoneme(word) = actual else {
            panic!("phonemic owner")
        };
        assert!(core::ptr::eq(*word, original));
    }
    assert!(matches!(
        prepare_pronunciation_plan_coverage(&order, &refs, &composite, &witnesses[..9]),
        Err(SpeechPlanCoverageRefusal::PhoneCount)
    ));
    let role_refs = roles.iter().collect::<Vec<_>>();
    let committed =
        conduit_speech::committed_plan_coverage::prepare_committed_speech_plan_coverage(
            &coverage, &role_refs,
        )
        .unwrap();
    assert!(core::ptr::eq(committed.coverage().intent(), &composite));
    assert_eq!(committed.commitments().len(), 2);
    assert!(matches!(
        conduit_speech::committed_plan_coverage::prepare_committed_speech_plan_coverage(
            &coverage,
            &role_refs[..1]
        ),
        Err(conduit_speech::committed_plan_coverage::CommittedPlanCoverageRefusal::MissingCommitment)
    ));
    let swapped = [role_refs[1], role_refs[0]];
    assert!(matches!(
        conduit_speech::committed_plan_coverage::prepare_committed_speech_plan_coverage(&coverage, &swapped),
        Err(
            conduit_speech::committed_plan_coverage::CommittedPlanCoverageRefusal::ForeignCommitment {
                word: 0
            }
        )
    ));
    if let Ok(path) = std::env::var("CONDUIT_PHONEMIC_COMMITTED_OUTPUT") {
        let evidence = serde_json::json!({
            "schema":"speech/committed-phonemic-coverage@1",
            "scope":"Original retained four-revision Language commitments through new phonemic pronunciation and existing complete segment coverage; no PCM/neural/playback authority",
            "original_commitments":material["commitments"],
            "source_revision_history_bytes":material["source_revision_history_bytes"],
            "final_runtime_bytes":snapshot["beam_bytes"],
            "lexical_tape_bytes":case.lexical.tape().clone().encode().unwrap(),
            "phonemic_profile_bytes":profile.clone().encode().unwrap(),
            "phonemic_word_requests":prepared.iter().map(|p|p.request_canonical().to_vec()).collect::<Vec<_>>(),
            "phonemic_source_executions":prepared.iter().flat_map(|p|p.executions()).map(|e|serde_json::json!({"program":e.source_program_hex(),"input":e.input_canonical(),"output":e.output_canonical()})).collect::<Vec<_>>(),
            "original_word_intent_bytes":words.iter().map(|w|w.intent().clone().encode().unwrap()).collect::<Vec<_>>(),
            "original_utterance_intent_bytes":composite.clone().encode().unwrap(),
            "complete_spoken_order_bytes":coverage.order().native().clone().encode().unwrap(),
            "complete_layout_bytes":layout.clone().encode().unwrap(),
            "composition_witness_bytes":witnesses.iter().map(|w|w.clone().encode().unwrap()).collect::<Vec<_>>(),
            "commitments":committed.commitments().len(), "segments":coverage.phone_events().len(),
            "common_ir_completion":false,"neural_projection":false,"played_frames":0
        });
        std::fs::write(path, serde_json::to_vec(&evidence).unwrap()).unwrap();
    }
}
