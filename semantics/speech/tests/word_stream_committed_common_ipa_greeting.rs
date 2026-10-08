#![cfg(all(feature = "semantic-bindings", feature = "kernel"))]
#[path = "common/vocative_language.rs"]
#[allow(dead_code)]
mod language;
use conduit_language::{lexical::prepare_lexical_tape, *};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_speech::{
    phonemic_pronunciation::*, phonemic_pronunciation_intent::*, plan_coverage::*, semantic::*,
};
#[path = "common/committed_greeting_components.rs"]
mod components_fixture;
#[path = "common/committed_greeting_inventory.rs"]
mod inventory_fixture;
#[path = "common/committed_greeting_pcm.rs"]
mod pcm_evidence;
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
fn actual_committed_common_greeting_preserves_ipa_syllable_and_context_custody() {
    run_greeting(false);
}
#[test]
#[ignore = "requires original retained four-revision Language commitments"]
fn actual_committed_common_greeting_projects_committed_linguistic_pitch() {
    run_greeting(true);
}
fn run_greeting(linguistic: bool) {
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
    use conduit_speech::{
        allophone_selection::*, declared_context::ExplicitAllophoneContext, ipa_inventory::*,
        ipa_shared::*, shared_intent::*,
    };
    let provenance = profile.provenance().clone();
    let inventory_fixture::InventoryFixture {
        inventory: linguistic_inventory,
        notation,
        phone_notations,
        phoneme_notations,
    } = inventory_fixture::inventory(&composite, &provenance);
    let phone_id = |ipa: &str| PhoneId::new(format!("phone/{ipa}")).unwrap();
    let admitted_ipa = PreparedIpaInventory::prepare(
        &linguistic_inventory,
        &notation,
        notation.variety(),
        notation.revision(),
        &phone_notations,
        &phoneme_notations,
    )
    .unwrap();
    let text = case.lexical.tape().source().material();
    let components_fixture::ComponentsFixture {
        phone_sequence,
        phoneme_sequence,
        correspondences,
        syllables,
        context,
        positions,
    } = components_fixture::components(&composite, text, &provenance, notation.variety());
    let segments = composite
        .events()
        .iter()
        .filter_map(|e| {
            if let SpeechUtteranceIntentEvent::Segment(s) = e {
                Some(s)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let shared = PreparedSpeechUtteranceIntent::prepare(
        &composite,
        SpeechIntentComponents {
            context: &context,
            intended_text: Some(text),
            phonemes: &phoneme_sequence,
            phones: &phone_sequence,
            morphemes: &[],
            morpheme_texts: &[],
            syllables: &syllables,
            correspondences: &correspondences,
        },
    )
    .unwrap();
    let joined = PreparedIpaSpeechUtteranceIntent::prepare(&shared, &admitted_ipa).unwrap();
    assert!(core::ptr::eq(
        joined.shared().original(),
        committed.coverage().intent()
    ));
    assert_eq!(joined.phonemes().len(), 10);
    let policy = SpeechAllophoneChoicePolicy::new(true, false, false, false, true, false).unwrap();
    let unspecified_prosody = SpeechProsodicContextSpecification::Unspecified;
    let careful = SpeechCarefulStyleSpecification::known(false).unwrap();
    let choices = positions
        .iter()
        .enumerate()
        .map(|(i, position)| {
            select_intent_allophone(
                &composite,
                i,
                &linguistic_inventory,
                &policy,
                ExplicitAllophoneContext {
                    syllable_position: position,
                    prosodic_context: &unspecified_prosody,
                    careful_style: &careful,
                },
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(choices[4].selected_phone(), Some(&phone_id("tʰ")));

    use conduit_audio::*;
    use conduit_speech::*;
    let anchor = AudioTrajectoryAnchor::new(
        AudioOriginIdentity::new(4907).unwrap(),
        AudioTimelineIdentity::new(5218).unwrap(),
    )
    .unwrap();
    let grid = AudioSampleRateBasis::new(anchor.clone(), AudioFrameQuantization::Floor, 16000)
        .unwrap()
        .encode()
        .unwrap();
    let mut durations = Vec::new();
    let mut cycles = Vec::new();
    for segment in &segments {
        let SpeechDurationSpecification::Known(d) = segment.prosody().duration() else {
            panic!("explicit duration required")
        };
        durations.push(
            speech_duration_to_audio(
                &SpeechExactDuration::new(*d.denominator(), *d.numerator_seconds())
                    .unwrap()
                    .encode()
                    .unwrap(),
            )
            .unwrap(),
        );
        let SpeechCycleSpecification::Known(c) = segment.prosody().fundamental_cycle() else {
            panic!("explicit cycle required")
        };
        cycles.push(
            speech_cycle_to_audio(
                &SpeechFundamentalCycle::new(*c.denominator(), *c.numerator_seconds())
                    .unwrap()
                    .encode()
                    .unwrap(),
            )
            .unwrap(),
        );
    }
    // This explicitly selected bounded profile requires uniform durations.
    for d in &durations {
        assert_eq!(d.admitted_canonical(), durations[0].admitted_canonical());
    }
    let d = durations[0].original();
    let fraction = |ordinal: u64| {
        AudioTimeFraction::new(
            *d.denominator(),
            d.numerator_seconds().checked_mul(ordinal).unwrap(),
        )
        .unwrap()
    };
    let total = fraction(u64::try_from(segments.len()).unwrap());
    let timings = (0..segments.len())
        .map(|i| {
            SpeechGestureTiming::new(
                anchor.clone(),
                fraction(0),
                total.clone(),
                fraction(i as u64 + 1),
                fraction(i as u64),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let gestures = choices
        .iter()
        .zip(&timings)
        .map(|(choice, timing)| {
            conduit_speech::ipa_gestures::prepare_ipa_contextual_greeting_phone_gestures(
                &joined,
                choice,
                timing,
                SpeechGreetingLossPolicy::AcceptLateralRhoticAndStepDiphthongApproximationV2,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    // Prepare every phone before generating the first sample.
    let plans = gestures
        .iter()
        .zip(&cycles)
        .map(|(gesture, cycle)| {
            prepare_greeting_renderer(
                gesture.contextual().profile(),
                &grid,
                cycle.admitted_canonical(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    if linguistic {
        include!("common/committed_greeting_linguistic_body.rs");
        return;
    }
    let pcm_evidence::PcmEvidence {
        samples,
        frame_counts,
        source_programs,
    } = pcm_evidence::render(&plans);

    if let Ok(path) = std::env::var("CONDUIT_PHONEMIC_COMMITTED_OUTPUT") {
        let evidence = serde_json::json!({
            "schema":"speech/committed-phonemic-coverage@1",
            "common_ipa_carrier":{
                "inventory_bytes":linguistic_inventory.clone().encode().unwrap(),
                "notation_profile_bytes":notation.clone().encode().unwrap(),
                "phone_notation_binding_bytes":phone_notations.iter().map(|v|v.clone().encode().unwrap()).collect::<Vec<_>>(),
                "phoneme_notation_binding_bytes":phoneme_notations.iter().map(|v|v.clone().encode().unwrap()).collect::<Vec<_>>(),
                "phoneme_sequence_bytes":phoneme_sequence.clone().encode().unwrap(),
                "phone_sequence_bytes":phone_sequence.clone().encode().unwrap(),
                "syllable_bytes":syllables.iter().map(|v|v.clone().encode().unwrap()).collect::<Vec<_>>(),
                "correspondence_bytes":correspondences.iter().map(|v|v.clone().encode().unwrap()).collect::<Vec<_>>(),
                "context_bytes":context.clone().encode().unwrap(),
                "allophone_choice_state_bytes":choices.iter().map(|v|v.state().clone().encode().unwrap()).collect::<Vec<_>>(),
                "profile_scope":"Explicit reviewed greeting, one-to-one realization, supplied syllables, selected static200Hz/0.2s per segment; no rich prosody or continuous coarticulation completion",
                "original_carrier_is_committed":true,
                "joined_original_pointer_assertion":true,
            },

            "scope":"Original retained four-revision Language commitments through new phonemic pronunciation and existing complete segment coverage; bounded formant PCM; no neural/playback authority",
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
            "formant_projection":{"rate_hz":16000,"frames":samples.len(),"frame_counts":frame_counts,"grid_bytes":grid,"timing_bytes":timings.iter().map(|t|t.clone().encode().unwrap()).collect::<Vec<_>>(),"cycle_bytes":cycles.iter().map(|c|c.admitted_canonical()).collect::<Vec<_>>(),"quantity_executions":cycles.iter().flat_map(|c|c.executions()).chain(durations.iter().flat_map(|d|d.executions())).map(|e|serde_json::json!({"program":e.source_program_hex(),"input":e.input_canonical(),"output":e.output_canonical()})).collect::<Vec<_>>(),"loss_policy":"AcceptLateralRhoticAndStepDiphthongApproximationV2","continuous_coarticulation":false},
            "gesture_projection":gestures.iter().map(|g|{
                let p=g.contextual().profile();let l=p.lowered();
                serde_json::json!({"original_frames":l.original_canonical_frames(),"admitted_gesture_frames":l.admitted_canonical_frames(),"profile":l.profile_identity(),"policy_frame":p.selected_policy_canonical(),"effect_frame":p.effect_canonical(),"policy_request":p.original_policy_request_canonical(),"admitted_policy_frame":p.admitted_policy_canonical(),"source_executions":l.executions().iter().map(|e|serde_json::json!({"program":e.source_program_hex(),"input":e.input_canonical(),"output":e.output_canonical()})).collect::<Vec<_>>()})
            }).collect::<Vec<_>>(),
            "frame_control_programs":source_programs,
            "dsp_graph":{"programs":conduit_speech::gesture_renderer::speech_gesture_dsp_programs::PROGRAMS,"inputs":conduit_speech::gesture_renderer::speech_gesture_dsp_programs::INPUTS,"connections":conduit_speech::gesture_renderer::speech_gesture_dsp_programs::CONNECTIONS,"result":conduit_speech::gesture_renderer::speech_gesture_dsp_programs::RESULT},
            "common_ir_completion":false,"neural_projection":false,"played_frames":0
        });
        std::fs::write(path, serde_json::to_vec(&evidence).unwrap()).unwrap();
    }
}
