#![cfg(all(feature = "kernel", feature = "semantic-bindings"))]
//! Supplied committed structures, exact authored pronunciations; no learned parser claim.
#[allow(dead_code)]
#[path = "common/vocative/carrier.rs"]
mod carrier;
#[path = "../../language/tests/common/committed_prosody.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "common/vocative/installed.rs"]
mod installed;
// Reuse the accepted fixture API; its other public reexports are unused here.
#[allow(unused_imports)]
#[path = "common/record_pronunciation/lexical.rs"]
mod record_lexical;
use conduit_core::revision::*;
use conduit_language::{
    analysis_session::*, dependency_commitment::PreparedCommittedDependency, lexical::*,
    prosody::*, prosody_commitment::*, revision::*, *,
};
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{committed_prosody::*, complete_carrier::*, semantic::*};
#[allow(dead_code)]
#[path = "common/vocative/revision.rs"]
mod revision;
use revision::*;

#[path = "common/vocative/acoustic_windows.rs"]
mod acoustic_windows;
/// A controlled authored acoustic comparison, not a listening-quality verdict.
#[test]
fn record_profile_cycle_compares_actual_installed_pcm_without_relabeling_baseline() {
    let execution = fixture::Prepared::new();
    let pronunciation_profile =
        conduit_speech::record_pronunciation::PreparedRecordPronunciationProfile::new().unwrap();
    let record_inventory = carrier::inventory::reviewed();
    let admitted_inventory = conduit_speech::ipa_inventory::PreparedIpaInventory::prepare(
        &record_inventory.inventory,
        &record_inventory.profile,
        record_inventory.profile.variety(),
        record_inventory.profile.revision(),
        &record_inventory.phones,
        &record_inventory.phonemes,
    )
    .unwrap();
    let (sentence, dep, head) = ("we record Ed", 2, 1);
    {
        let mut f = fixture::prosody::fixture(sentence, dep, head);
        let accepted_lexical = record_lexical::lexical("we record");
        let mut entries = accepted_lexical
            .tape()
            .profile()
            .entries()
            .as_slice()
            .to_vec();
        entries.push(
            LanguageLexicalEntry::new(
                BoundedSequence::try_from_iter([LanguageLexicalCandidate::new(
                    "Ed".into(),
                    BoundedSequence::new(),
                    LanguageLexicalPos::ProperNoun,
                )
                .unwrap()])
                .unwrap(),
                "Ed".into(),
            )
            .unwrap(),
        );
        let profile = LanguageLexicalProfile::new(
            BoundedSequence::try_from_iter(entries).unwrap(),
            "authored/Ed-plus-accepted-record@1".into(),
            f.profile.language().clone(),
            fixture::prosody::provenance(),
        )
        .unwrap();
        let source = LanguageTextRevision::new(
            LanguageTextFinality::Final,
            f.lexical.tape().source().material().clone(),
            None,
            fixture::prosody::provenance(),
            0,
            Some(sentence.chars().count() as u32),
        )
        .unwrap();
        f.lexical = prepare_lexical_tape(&source, &profile, None).unwrap();
        f.discourse = conduit_language::discourse::prepare_vocative_fact(
            "authored/vocative".into(),
            &source,
            f.discourse.fact().analysis(),
            f.discourse.fact().basis(),
            fixture::prosody::provenance(),
            3,
        )
        .unwrap();
        let raw_request = fixture::request(&f, 3, false, false);
        let subject = f
            .lexical
            .tape()
            .tokens()
            .iter()
            .position(|token| token.surface() == "we")
            .unwrap();
        let record_ordinal = f
            .lexical
            .tape()
            .tokens()
            .iter()
            .position(|token| token.surface() == "record")
            .unwrap();
        let request = with_subject(&raw_request, subject);
        let subject_arc = LanguageDependencyArc::new(
            LanguageAnalysisTokenRef::new(
                f.discourse.fact().analysis().clone(),
                f.lexical.tape().tokens()[subject].identity().clone(),
            )
            .unwrap(),
            LanguageDependencyHead::token(
                f.discourse.fact().analysis().clone(),
                f.lexical.tape().tokens()[record_ordinal].identity().clone(),
            )
            .unwrap(),
            LanguageDependencyRelation::new(LanguageUniversalDependencyRelation::Nsubj, None)
                .unwrap(),
        )
        .unwrap();
        let selection = conduit_language::pronunciation_selection::prepare_pronunciation_selection(
            &f.lexical,
            record_ordinal,
            f.discourse.fact().analysis(),
            &subject_arc,
            &record_lexical::profile(),
        )
        .unwrap();
        let literal = pronunciation_profile
            .project_supplied_selection(&selection, &admitted_inventory)
            .unwrap();
        assert_eq!(literal.dictionary_ipa(), "rɪˈkɔːd");
        assert_eq!(literal.phonetic().transcription().original(), "ɹɪˈkɔːd");
        assert_eq!(literal.stressed_syllable(), 1);
        let result = fixture::result(&execution, &request);
        PreparedCommittedDependency::prepare(&f.lexical, &subject_arc, &request, &result).unwrap();

        let symbolic = LanguageProsodySymbolicProfile::new(
            f.profile.clone(), LanguageProsodyDurationIntent::Neutral,
            LanguageProsodyFocusIntent::Neutral, LanguageProsodyRateIntent::Neutral,
        ).unwrap();
        let committed = PreparedCommittedProsody::prepare(
            &f.lexical, dep as usize, f.discourse.fact(), &symbolic, &request, &result,
        ).unwrap();
        let rich = prepare_rich_prosody(&f.lexical, dep as usize, f.discourse.fact(), &f.profile).unwrap();
        with_current(committed.dependency(), &f.lexical, |current| {
            let guard = CommittedSpeechProsody::prepare(&committed, &rich, current).unwrap();
            let baseline = carrier::build(f.lexical.tape().source(), &profile, dep as usize, None, &literal);
            let variant = carrier::build_with_record_profile_cycle(f.lexical.tape().source(), &profile, dep as usize, None, &literal);
            assert_eq!(baseline.formant, variant.formant);
            assert_eq!(baseline.phones, variant.phones);
            assert_eq!(baseline.syllables, variant.syllables);
            assert_eq!(baseline.intent.events().len(), variant.intent.events().len());
            let record_start = *f.lexical.tape().tokens()[record_ordinal].span().start();
            let is_record = |segment: &SpeechUtteranceIntentSegment| segment.sources().iter().any(|source| {
                matches!(source, LanguageSegmentRef::Text(reference) if u64::from(*reference.range().start()) == record_start)
            });
            for (old, new) in baseline.intent.events().iter().zip(variant.intent.events().iter()) {
                match (old, new) {
                    (SpeechUtteranceIntentEvent::Segment(old), SpeechUtteranceIntentEvent::Segment(new)) if is_record(old) => {
                        assert_eq!(old.phone(), new.phone()); assert_eq!(old.phoneme(), new.phoneme());
                        assert_eq!(old.stress(), new.stress()); assert_eq!(old.sources(), new.sources());
                        assert_eq!(old.prosody().duration(), new.prosody().duration());
                        assert_eq!(old.prosody().relative_intensity(), new.prosody().relative_intensity());
                        assert_eq!(old.prosody().fundamental_cycle(), &SpeechCycleSpecification::known(120, 1).unwrap());
                        assert_eq!(new.prosody().fundamental_cycle(), &SpeechCycleSpecification::unspecified());
                    }
                    _ => assert_eq!(old, new, "Ed/we and all boundaries stay exact"),
                }
            }
            let mut measured = Vec::new();
            let mut last_pcm = Vec::new();
            for (label, specimen) in [("record-baseline-fixed-120", baseline), ("record-variant-profile-cycle", variant)] {
                let inventory = specimen.inventory().unwrap();
                let correspondence = specimen.correspondence(&inventory).unwrap();
                let structure = conduit_speech::syllable_structure::PreparedSyllableStructure::prepare(
                    &specimen.intent, &specimen.phones, specimen.source.material(),
                    &specimen.syllables, &specimen.positioned_boundaries,
                ).unwrap();
                let realization = conduit_speech::intent_realization::prepare_intent_realization(
                    &specimen.intent, &specimen.definitions, &specimen.formant, &specimen.boundaries,
                ).unwrap();
                let record_events = specimen.intent.events().iter().enumerate().filter_map(|(event, value)| match value {
                    SpeechUtteranceIntentEvent::Segment(segment) if is_record(segment) => Some(event), _ => None,
                }).collect::<Vec<_>>();
                assert_eq!(record_events.len(), 5);
                let ih = &realization.timing().spans()[record_events[1]];
                let ao = &realization.timing().spans()[record_events[3]];
                assert_eq!(*ih.frame_count(), 800); assert_eq!(*ao.frame_count(), 1600);
                let complete = PreparedCompleteSpeechCarrier::prepare(
                    &correspondence, &structure, &f.lexical, &specimen.word_phonemes,
                    &request, &result, CompleteCarrierAcoustics::not_supplied(),
                ).unwrap();
                let pcm = installed::execute_control(&guard, &complete, &specimen.formant,
                    &specimen.boundaries, carrier::inventory::provenance(), label);
                assert_eq!(pcm.len(), 16000);
                let ih_window = acoustic_windows::Window::center(*ih.start_frame() as usize, *ih.frame_count() as usize);
                let ao_window = acoustic_windows::Window::center(*ao.start_frame() as usize, *ao.frame_count() as usize);
                let ih_pitch = acoustic_windows::measure(&pcm, ih_window);
                let ao_pitch = acoustic_windows::measure(&pcm, ao_window);
                let row = serde_json::json!({
                    "label":label,"sample_rate_hz":8000,"total_frames":pcm.len()/2,
                    "ih_frames":ih.frame_count(),"ao_frames":ao.frame_count(),
                    "ih_window":ih_window.json(),"ao_window":ao_window.json(),
                    "ih":ih_pitch.json(),"ao":ao_pitch.json(),
                    "listening_quality":"unassessed; authored acoustic comparison only",
                    "default_profile_source_period_samples":{"primary_start":58,"primary_end":66,"unstressed_start":67,"unstressed_end":69}
                });
                println!("record acoustic {row}");
                if let Some(root) = std::env::var_os("CONDUIT_VOCATIVE_PROOF_DIR") {
                    std::fs::write(std::path::PathBuf::from(root).join(format!("{label}.acoustics.json")), serde_json::to_vec_pretty(&row).unwrap()).unwrap();
                }
                measured.push((ih_pitch, ao_pitch)); last_pcm = pcm;
            }
            // Independent waveform estimates: explicit 120Hz should remain near
            // 67 samples; the profile's second-syllable AO has a shorter period.
            for pitch in [measured[0].0, measured[0].1] { assert!((65..=69).contains(&pitch.lag), "{pitch:?}"); }
            assert!((66..=70).contains(&measured[1].0.lag), "{:?}", measured[1].0);
            assert!((58..=66).contains(&measured[1].1.lag), "{:?}", measured[1].1);
            assert!(measured[1].0.lag >= measured[1].1.lag + 2);
            last_pcm
        });
    }
}
