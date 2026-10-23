#![cfg(all(feature = "kernel", feature = "semantic-bindings"))]
//! Supplied committed structures, exact authored pronunciations; no learned parser claim.
#[path = "common/vocative/carrier.rs"]
mod carrier;
#[path = "../../language/tests/common/committed_prosody.rs"]
mod fixture;
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
#[path = "common/vocative/revision.rs"]
mod revision;
use revision::*;
#[test]
fn three_committed_vocative_placements_realize_exact_carrier_and_pcm() {
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
    for (sentence, dep, head) in [
        ("Ed we record", 0, 2),
        ("we Ed record", 1, 2),
        ("we record Ed", 2, 1),
    ] {
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
        let c = request.beam().candidate0();
        let alternate = LanguageParserBeamCommitRequest::new(
            LanguageParserAdmittedBeam::new(
                request.beam().basis().clone(),
                LanguageParserHypothesis::new(*c.active(), 41, *c.score(), c.state().clone())
                    .unwrap(),
                request.beam().candidate1().clone(),
                request.beam().candidate2().clone(),
                request.beam().candidate3().clone(),
            )
            .unwrap(),
            false,
            request.frontier().clone(),
            false,
            *request.through(),
        )
        .unwrap();
        let alternate_result = fixture::result(&execution, &alternate);
        let symbolic = LanguageProsodySymbolicProfile::new(
            f.profile.clone(),
            LanguageProsodyDurationIntent::Neutral,
            LanguageProsodyFocusIntent::Neutral,
            LanguageProsodyRateIntent::Neutral,
        )
        .unwrap();
        let committed = PreparedCommittedProsody::prepare(
            &f.lexical,
            dep as usize,
            f.discourse.fact(),
            &symbolic,
            &request,
            &result,
        )
        .unwrap();
        let rich =
            prepare_rich_prosody(&f.lexical, dep as usize, f.discourse.fact(), &f.profile).unwrap();
        with_current(committed.dependency(), &f.lexical, |current| {
            let guard = CommittedSpeechProsody::prepare(&committed, &rich, current).unwrap();
            let offered = binding(&f, &rich, 130);
            let acoustic = guard.prepare_acoustics(&offered).unwrap();
            let control_offer = binding(&f, &rich, 120);
            let pitch_control = guard.prepare_acoustics(&control_offer).unwrap();
            let duration = acoustic.prepare_formant_segment().unwrap();
            assert_eq!(*duration.spans()[0].frame_count(), 800);
            let doubled = conduit_speech::duration::duration_spans(
                std::slice::from_ref(duration.spans()[0].duration()),
                16000,
            )
            .unwrap();
            assert_eq!(*doubled[0].frame_count(), 1600);
            assert_eq!(acoustic.accepted().requested(), committed.intent().choice());
            let LanguageSegmentRef::Text(span) = &acoustic.boundary().sources()[0] else {
                panic!()
            };
            assert_eq!(
                span.text_id(),
                f.lexical.tape().source().material().identity()
            );
            assert_eq!(
                u64::from(*span.range().start()),
                *rich.requested().token().span().start()
            );
            for duplicate in [false, true] {
                let mut hostile = carrier::build(
                    f.lexical.tape().source(),
                    &profile,
                    dep as usize,
                    Some(&acoustic),
                    &literal,
                );
                let target = acoustic.boundary().sources();
                let other = hostile
                    .intent
                    .events()
                    .iter()
                    .find_map(|event| match event {
                        SpeechUtteranceIntentEvent::Segment(segment)
                            if segment.sources() != target =>
                        {
                            Some(segment.sources()[0].clone())
                        }
                        _ => None,
                    })
                    .unwrap();
                let mut events = hostile.intent.events().as_slice().to_vec();
                let index=events.iter().position(|event|matches!(event,SpeechUtteranceIntentEvent::Boundary(value) if value.sources()==target)).unwrap();
                if duplicate {
                    let boundary = events[index].clone();
                    events.insert(index, boundary);
                } else {
                    let SpeechUtteranceIntentEvent::Boundary(boundary) = &events[index] else {
                        panic!()
                    };
                    events[index] = SpeechUtteranceIntentEvent::boundary(
                        boundary.duration().clone(),
                        boundary.kind().clone(),
                        boundary.provenance().clone(),
                        BoundedSequence::try_from_iter([target[0].clone(), other]).unwrap(),
                    )
                    .unwrap();
                }
                hostile.intent = SpeechUtteranceIntent::new(
                    BoundedSequence::try_from_iter(events).unwrap(),
                    hostile.intent.inventory_id().clone(),
                    hostile.intent.language().clone(),
                    hostile.intent.provenance().clone(),
                    hostile.intent.revision_id().clone(),
                    hostile.intent.utterance_id().clone(),
                )
                .unwrap();
                let inventory = hostile.inventory().unwrap();
                let correspondence = hostile.correspondence(&inventory).unwrap();
                let structure =
                    conduit_speech::syllable_structure::PreparedSyllableStructure::prepare(
                        &hostile.intent,
                        &hostile.phones,
                        hostile.source.material(),
                        &hostile.syllables,
                        &hostile.positioned_boundaries,
                    )
                    .unwrap();
                let complete = PreparedCompleteSpeechCarrier::prepare(
                    &correspondence,
                    &structure,
                    &f.lexical,
                    &hostile.word_phonemes,
                    &request,
                    &result,
                    CompleteCarrierAcoustics::not_supplied(),
                )
                .unwrap();
                assert!(matches!(
                    guard.bind_carrier(&complete, &acoustic),
                    Err(CommittedSpeechProsodyRefusal::CarrierAcoustic)
                ));
            }
            let mut pcms = std::collections::BTreeMap::new();
            for (variant, selected) in [(2, Some(&acoustic)), (0, None), (1, Some(&pitch_control))]
            {
                let label = format!("placement-{dep}-variant-{variant}");
                let specimen = carrier::build(
                    f.lexical.tape().source(),
                    &profile,
                    dep as usize,
                    selected,
                    &literal,
                );
                let inventory = specimen.inventory().unwrap();
                let correspondence = specimen.correspondence(&inventory).unwrap();
                let structure =
                    conduit_speech::syllable_structure::PreparedSyllableStructure::prepare(
                        &specimen.intent,
                        &specimen.phones,
                        specimen.source.material(),
                        &specimen.syllables,
                        &specimen.positioned_boundaries,
                    )
                    .unwrap();
                let realization = conduit_speech::intent_realization::prepare_intent_realization(
                    &specimen.intent,
                    &specimen.definitions,
                    &specimen.formant,
                    &specimen.boundaries,
                )
                .unwrap();
                let record_segments:Vec<_>=specimen.intent.events().iter().enumerate().filter_map(|(event,value)|match value {SpeechUtteranceIntentEvent::Segment(segment) if segment.sources().iter().any(|source|matches!(source,LanguageSegmentRef::Text(reference) if u64::from(*reference.range().start())==*f.lexical.tape().tokens()[record_ordinal].span().start()))=>Some((event,segment)),_=>None}).collect();
                assert_eq!(record_segments.len(), 5);
                let (ao_event, ao) = record_segments[3];
                assert_eq!(
                    ao.stress(),
                    &StressSpecification::known(SpeechStress::Primary).unwrap()
                );
                assert_eq!(*realization.timing().spans()[ao_event].frame_count(), 1600);
                assert_eq!(
                    *realization.timing().spans()[record_segments[1].0].frame_count(),
                    800
                );
                let conduit_speech::VoiceEvent::phone(compact) = realization.events()[ao_event]
                else {
                    panic!("record AO must reach compact phone renderer")
                };
                assert_eq!(compact.phone, conduit_speech::EnglishPhone::ao);
                assert_eq!(compact.stress, conduit_speech::EnglishStress::primary);
                let complete = PreparedCompleteSpeechCarrier::prepare(
                    &correspondence,
                    &structure,
                    &f.lexical,
                    &specimen.word_phonemes,
                    &request,
                    &result,
                    CompleteCarrierAcoustics::not_supplied(),
                )
                .unwrap();
                let foreign = PreparedCompleteSpeechCarrier::prepare(
                    &correspondence,
                    &structure,
                    &f.lexical,
                    &specimen.word_phonemes,
                    &alternate,
                    &alternate_result,
                    CompleteCarrierAcoustics::not_supplied(),
                )
                .unwrap();
                assert!(matches!(
                    guard.bind_carrier(&foreign, &acoustic),
                    Err(CommittedSpeechProsodyRefusal::CarrierCommitment)
                ));
                let pcm = if let Some(selected) = selected {
                    let bound = guard.bind_carrier(&complete, selected).unwrap();
                    installed::execute(
                        &bound,
                        &specimen.formant,
                        &specimen.boundaries,
                        carrier::inventory::provenance(),
                        &label,
                    )
                } else {
                    assert!(matches!(
                        guard.bind_carrier(&complete, &acoustic),
                        Err(CommittedSpeechProsodyRefusal::CarrierAcoustic)
                    ));
                    installed::execute_control(
                        &guard,
                        &complete,
                        &specimen.formant,
                        &specimen.boundaries,
                        carrier::inventory::provenance(),
                        &label,
                    )
                };
                pcms.insert(variant, pcm);
            }
            let mut pcms: Vec<Vec<u8>> = pcms.into_values().collect();
            assert_eq!(pcms[1].len() - pcms[0].len(), 240 * 2);
            assert_eq!(pcms[1].len(), pcms[2].len());
            assert_ne!(pcms[1], pcms[2]); // Same boundary/duration/intensity, pitch only.
            println!("vocative source={sentence:?} placement={dep} source_span={:?} neutral_frames={} phrase_frames={} selected_frames={} pitch_hz=120/130", rich.requested().token().span(),pcms[0].len()/2,pcms[1].len()/2,pcms[2].len()/2);
            let unsupported = PreparedCommittedProsody::prepare(
                &f.lexical,
                dep as usize,
                f.discourse.fact(),
                &fixture::symbolic(&f.profile),
                &request,
                &result,
            )
            .unwrap();
            let unsupported =
                CommittedSpeechProsody::prepare(&unsupported, &rich, current).unwrap();
            assert!(matches!(
                unsupported.prepare_acoustics(&offered),
                Err(CommittedSpeechProsodyRefusal::UnsupportedSymbolicControl)
            ));
            pcms.pop().unwrap()
        });
    }
}

#[test]
fn comma_without_vocative_edge_cannot_gain_rich_receipt() {
    let f = fixture::prosody::fixture("Ed, we record", 0, 3);
    let arc = f.discourse.fact().basis();
    let missing = LanguageDependencyArc::new(
        arc.dependent().clone(),
        arc.governor().clone(),
        LanguageDependencyRelation::new(LanguageUniversalDependencyRelation::Dep, None).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        conduit_language::discourse::prepare_vocative_fact(
            "comma-only".into(),
            f.lexical.tape().source(),
            arc.dependent().revision(),
            &missing,
            fixture::prosody::provenance(),
            f.lexical.tape().tokens().len() as u64
        ),
        Err(conduit_language::discourse::DiscourseRefusal::Native(_))
    ));
    // This control never enters the all-word acoustic carrier. Source is not stripped.
    assert!(f.lexical.tape().source().material().text().contains(','));
}
