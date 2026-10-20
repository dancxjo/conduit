#![cfg(feature = "semantic-bindings")]
#[path = "common/record_pronunciation/inventory.rs"]
mod inventory;
#[allow(dead_code)]
#[path = "common/record_pronunciation/lexical.rs"]
mod lexical;
use conduit_language::{pronunciation_selection::*, *};
use conduit_speech::{ipa_inventory::PreparedIpaInventory, record_pronunciation::*};
#[test]
fn supplied_syntax_selects_different_canonical_record_phones_without_claiming_commitment() {
    let owned = inventory::reviewed();
    let admitted = PreparedIpaInventory::prepare(
        &owned.inventory,
        &owned.profile,
        owned.profile.variety(),
        owned.profile.revision(),
        &owned.phones,
        &owned.phonemes,
    )
    .unwrap();
    let profile = PreparedRecordPronunciationProfile::new().unwrap();
    for (text, relation, dictionary, phone, vowel, stress) in [
        (
            "a record",
            LanguageUniversalDependencyRelation::Det,
            "ˈrek.ɔːd",
            "ˈɹɛk.ɔːd",
            "ɛ",
            0,
        ),
        (
            "we record",
            LanguageUniversalDependencyRelation::Nsubj,
            "rɪˈkɔːd",
            "ɹɪˈkɔːd",
            "ɪ",
            1,
        ),
    ] {
        let tape = lexical::lexical(text);
        let selected = prepare_pronunciation_selection(
            &tape,
            1,
            &lexical::analysis(),
            &lexical::arc(&tape, relation),
            &lexical::profile(),
        )
        .unwrap();
        let receipt = profile
            .project_supplied_selection(&selected, &admitted)
            .unwrap();
        assert!(core::ptr::eq(receipt.selection(), &selected));
        assert_eq!(
            receipt.selection().request().source().material().text(),
            text
        );
        assert_eq!(receipt.dictionary_ipa(), dictionary);
        assert_eq!(receipt.phonetic().transcription().original(), phone);
        assert_eq!(receipt.first_vowel_ipa(), vowel);
        assert_eq!(receipt.stressed_syllable(), stress);
        assert_eq!(*receipt.long_duration().numerator_seconds(), 1);
        assert_eq!(*receipt.long_duration().denominator(), 5);
        assert_eq!(
            conduit_plot::PortableExpressionProgram::from_canonical_hex(profile.program_hex())
                .unwrap()
                .evaluate(receipt.input_canonical())
                .unwrap(),
            receipt.output_canonical()
        );
    }
}
#[test]
fn foreign_variety_refuses_instead_of_falling_back_or_reinterpreting_ipa() {
    let owned = inventory::inventory("foreign/variety");
    let admitted = PreparedIpaInventory::prepare(
        &owned.inventory,
        &owned.profile,
        owned.profile.variety(),
        owned.profile.revision(),
        &owned.phones,
        &owned.phonemes,
    )
    .unwrap();
    let tape = lexical::lexical("a record");
    let selection = prepare_pronunciation_selection(
        &tape,
        1,
        &lexical::analysis(),
        &lexical::arc(&tape, LanguageUniversalDependencyRelation::Det),
        &lexical::profile(),
    )
    .unwrap();
    assert!(matches!(
        PreparedRecordPronunciationProfile::new()
            .unwrap()
            .project_supplied_selection(&selection, &admitted),
        Err(RecordPronunciationRefusal::UnsupportedProfile)
    ));
}

#[test]
fn only_the_record_target_changes_compact_pcm_not_a_different_prefix() {
    use conduit_audio::{
        AudioFrameQuantization, AudioOriginIdentity, AudioSampleProjectionQuantity,
        AudioSampleProjectionRequest, AudioSampleRateBasis, AudioTimelineIdentity,
        AudioTrajectoryAnchor, PreparedAudioSampleProjection,
    };
    use conduit_speech::{
        EnglishPhoneme as P, EnglishPosition, EnglishStress, RealizationInput, Renderer, VoiceEvent,
    };
    let owned = inventory::reviewed();
    let admitted = PreparedIpaInventory::prepare(
        &owned.inventory,
        &owned.profile,
        owned.profile.variety(),
        owned.profile.revision(),
        &owned.phones,
        &owned.phonemes,
    )
    .unwrap();
    let profile = PreparedRecordPronunciationProfile::new().unwrap();
    let mut outputs = Vec::new();
    for (text, relation) in [
        ("a record", LanguageUniversalDependencyRelation::Det),
        ("we record", LanguageUniversalDependencyRelation::Nsubj),
    ] {
        let tape = lexical::lexical(text);
        let selection = prepare_pronunciation_selection(
            &tape,
            1,
            &lexical::analysis(),
            &lexical::arc(&tape, relation),
            &lexical::profile(),
        )
        .unwrap();
        let receipt = profile
            .project_supplied_selection(&selection, &admitted)
            .unwrap();
        let symbols: Vec<_> = receipt
            .phonetic()
            .transcription()
            .units()
            .as_slice()
            .iter()
            .filter(|u| {
                matches!(
                    u.value(),
                    conduit_speech::semantic::SpeechPhoneticEvent::Phone(_)
                )
            })
            .map(|u| u.source_spelling().get().as_str())
            .collect();
        // Explicit test realization table matches the reviewed profile. It does
        // not select pronunciation: those different targets came from Source.
        let phonemes: Vec<_> = symbols
            .iter()
            .map(|s| match *s {
                "ɹ" => P::r,
                "ɛ" => P::eh,
                "ɪ" => P::ih,
                "k" => P::k,
                "ɔ" => P::ao,
                "d" => P::d,
                _ => panic!("unsupported target"),
            })
            .collect();
        assert_eq!(phonemes.len(), 5); // Prefixes are deliberately excluded.
        let events: Vec<_> = phonemes
            .into_iter()
            .enumerate()
            .map(|(i, phoneme)| {
                VoiceEvent::segment(RealizationInput {
                    phoneme,
                    position: EnglishPosition::medial,
                    stress: if (i == 1 && receipt.stressed_syllable() == 0)
                        || (i == 3 && receipt.stressed_syllable() == 1)
                    {
                        EnglishStress::primary
                    } else if i == 1 || i == 3 {
                        EnglishStress::unstressed
                    } else {
                        EnglishStress::unspecified
                    },
                })
            })
            .collect();
        let anchor = AudioTrajectoryAnchor::new(
            AudioOriginIdentity::new(1).unwrap(),
            AudioTimelineIdentity::new(1).unwrap(),
        )
        .unwrap();
        let basis = AudioSampleRateBasis::new(anchor, AudioFrameQuantization::Floor, 8000).unwrap();
        let project = PreparedAudioSampleProjection::new().unwrap();
        let mut counts = Vec::new();
        for i in 0..5 {
            let duration = if i == 3 {
                receipt.long_duration()
            } else {
                receipt.short_duration()
            };
            let request = AudioSampleProjectionRequest::new(
                basis.clone(),
                AudioSampleProjectionQuantity::duration(
                    *duration.denominator(),
                    *duration.numerator_seconds(),
                )
                .unwrap(),
            )
            .unwrap();
            let output = project
                .project(&conduit_plot::rust_binding::NativeRustBinding::encode(request).unwrap())
                .unwrap();
            assert_eq!(*output.result().raw().remainder_numerator(), 0);
            counts.push(i32::try_from(*output.result().raw().whole_frames()).unwrap());
        }
        assert_eq!(counts, [800, 800, 800, 1600, 800]);
        let mut render = Renderer::prepare_timed(&events, &counts).unwrap();
        let mut pcm = Vec::new();
        let mut block = [0i16; 256];
        while !render.is_complete() {
            let n = render.render(&mut block).unwrap();
            pcm.extend_from_slice(&block[..n]);
        }
        assert_eq!(pcm.len(), 4800);
        outputs.push(pcm);
    }
    assert_ne!(outputs[0], outputs[1]);
}

#[test]
fn no_parser_route_is_explicitly_reduced_heuristic() {
    use conduit_speech::{VoiceBoundary, VoiceEvent, MAXIMUM_EVENTS};
    let mut storage = [VoiceEvent::boundary(VoiceBoundary::word); MAXIMUM_EVENTS];
    let fallback = prepare_reduced_heuristic("a record", &mut storage).unwrap();
    assert_eq!(fallback.route(), "reduced-heuristic-compact-text@1");
    assert_eq!(fallback.pronounced().source(), "a record");
    assert!(!fallback.pronounced().events().is_empty());
}
