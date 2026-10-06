//! Manual occurrence fixture: native contextual choice, not language commitment.
#[path = "global_rule.rs"]
mod global_rule;
use conduit_core::IeeeF32;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{
    allophone_selection::select_intent_allophone,
    chosen_allophone_profile::prepare_chosen_allophone_profile,
    declared_context::ExplicitAllophoneContext, semantic::*,
};
fn phone(id: &str, ipa: &str) -> SpeechPhone {
    SpeechPhone::new(
        BoundedSequence::new(),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        PhoneId::new(id.into()).unwrap(),
        ipa.into(),
        SpeechSegmentStatus::Core,
    )
    .unwrap()
}
fn phoneme(
    id: &str,
    default: &str,
    ipa: &str,
    allophones: Vec<SpeechPhonemeAllophone>,
) -> SpeechPhoneme {
    SpeechPhoneme::new(
        BoundedSequence::new(),
        BoundedSequence::try_from_iter(allophones).unwrap(),
        Some(PhoneId::new(default.into()).unwrap()),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        PhonemeId::new(id.into()).unwrap(),
        ipa.into(),
        BoundedSequence::new(),
        SpeechSegmentStatus::Core,
    )
    .unwrap()
}
pub fn write(output: &str) -> Result<(), Box<dyn std::error::Error>> {
    let inventory_id = SpeechInventoryId::new("choice/inventory".into()).unwrap();
    let language = LanguageId::new("en".into()).unwrap();
    let rule = SpeechPhonemeAllophone::new(
        BoundedSequence::new(),
        SpeechConfidence::new(IeeeF32::from_value(1.0)).unwrap(),
        SpeechEnvironment::new(
            BoundedSequence::new(),
            BoundedSequence::new(),
            SpeechProsodicContextSpecification::unspecified(),
            StressSpecification::unspecified(),
            SpeechSyllablePositionSpecification::unspecified(),
            SpeechPositionSpecification::known(SpeechWordPosition::Initial).unwrap(),
        )
        .unwrap(),
        PhoneId::new("choice/aspirated-t".into()).unwrap(),
        Some("manual word-initial aspiration".into()),
        SpeechRuleStatus::Productive,
    )
    .unwrap();
    let definitions = [
        phone("choice/t", "t"),
        phone("choice/aspirated-t", "tʰ"),
        phone("choice/aa", "ɑ"),
    ];
    let inventory = SpeechInventory::new(
        inventory_id.clone(),
        language.clone(),
        BoundedSequence::try_from_iter([
            phoneme("choice/phoneme/t", "choice/t", "t", vec![rule]),
            phoneme("choice/phoneme/aa", "choice/aa", "ɑ", vec![]),
        ])
        .unwrap(),
        BoundedSequence::try_from_iter(definitions.clone()).unwrap(),
    )
    .unwrap();
    let profile = SpeechFormantVoiceProfile::new(
        "manual choice profile".into(),
        inventory_id.clone(),
        language.clone(),
        BoundedSequence::try_from_iter(
            definitions
                .into_iter()
                .zip([EnglishPhone::T, EnglishPhone::TAspirated, EnglishPhone::Aa])
                .map(|(definition, realization)| {
                    SpeechFormantPhoneBinding::new(definition, realization).unwrap()
                }),
        )
        .unwrap(),
    )
    .unwrap();
    let provenance = SpeechEvidenceProvenance::new(
        "manual allophone choice fixture".into(),
        SpeechEvidenceSource::Manual,
        None,
    )
    .unwrap();
    let revision = SpeechSegmentRevisionId::new("choice/revision".into()).unwrap();
    let utterance = SpeechUtteranceId::new("choice/tata".into()).unwrap();
    let sources = BoundedSequence::try_from_iter([LanguageSegmentRef::text(
        LanguageTextSegmentKind::Word,
        language.clone(),
        LanguageTextRange::new(4, 0).unwrap(),
        LanguageTextRevisionId::new("choice/text-revision".into()).unwrap(),
        LanguageTextId::new("choice/text".into()).unwrap(),
    )
    .unwrap()])
    .unwrap();
    let events = [
        "choice/phoneme/t",
        "choice/phoneme/aa",
        "choice/phoneme/t",
        "choice/phoneme/aa",
    ]
    .into_iter()
    .enumerate()
    .map(|(index, id)| {
        SpeechUtteranceIntentEvent::segment(
            LanguageSpeechTokenRef::new(
                inventory_id.clone(),
                language.clone(),
                index as u32,
                revision.clone(),
                SpeechSegmentSequenceId::new("choice/sequence".into()).unwrap(),
                utterance.clone(),
            )
            .unwrap(),
            PhoneSpecification::unspecified(),
            PhonemeSpecification::known(PhonemeId::new(id.into()).unwrap()).unwrap(),
            SpeechSegmentProsodyIntent::new(
                SpeechDurationSpecification::known(if index % 2 == 0 { 12 } else { 6 }, 1).unwrap(),
                SpeechCycleSpecification::known(120, 1).unwrap(),
                SpeechIntensitySpecification::known(1, 1).unwrap(),
            )
            .unwrap(),
            provenance.clone(),
            sources.clone(),
            StressSpecification::known(SpeechStress::Primary).unwrap(),
            SpeechPositionSpecification::known(if index == 0 {
                SpeechWordPosition::Initial
            } else if index == 3 {
                SpeechWordPosition::Final
            } else {
                SpeechWordPosition::Medial
            })
            .unwrap(),
        )
        .unwrap()
    });
    let intent = SpeechUtteranceIntent::new(
        BoundedSequence::try_from_iter(events).unwrap(),
        inventory_id,
        language,
        provenance,
        revision,
        utterance,
    )
    .unwrap();
    let style = SpeechCarefulStyleSpecification::known(false).unwrap();
    let syllable = SpeechSyllablePositionSpecification::unknown();
    let prosody = SpeechProsodicContextSpecification::unknown();
    for (productive, name) in [
        (false, "allophone-default-tata"),
        (true, "allophone-initial-aspiration-tata"),
    ] {
        let policy =
            SpeechAllophoneChoicePolicy::new(true, false, false, false, productive, false).unwrap();
        let mut compact = Vec::new();
        for event in 0..4 {
            let choice = select_intent_allophone(
                &intent,
                event,
                &inventory,
                &policy,
                ExplicitAllophoneContext {
                    careful_style: &style,
                    syllable_position: &syllable,
                    prosodic_context: &prosody,
                },
            )
            .map_err(|e| format!("{e:?}"))?;
            let prepared = prepare_chosen_allophone_profile(&choice, &profile)
                .map_err(|e| format!("{e:?}"))?;
            println!(
                "{name} event {event}: {:?}, {:?}",
                choice.state().outcome(),
                choice.selected_phone()
            );
            compact.push(prepared.event());
        }
        super::write_wav(output, name, &compact)?;
        let contexts = [Some(ExplicitAllophoneContext {
            careful_style: &style,
            syllable_position: &syllable,
            prosodic_context: &prosody,
        }); 4];
        let boundaries = SpeechFormantBoundaryProfile::new(BoundedSequence::new()).unwrap();
        let combined = conduit_speech::contextual_intent_realization::prepare_contextual_intent(
            &intent,
            &inventory,
            &profile,
            &boundaries,
            &policy,
            &contexts,
        )
        .map_err(|e| format!("{e:?}"))?;
        super::write_rendered(
            output,
            &format!("contextual-intent-{name}"),
            combined.renderer().map_err(|e| format!("{e:?}"))?,
        )?;
        let text = LanguageText::new(
            LanguageTextId::new("choice/text".into()).unwrap(),
            intent.language().clone(),
            LanguageTextRevisionId::new("choice/text-revision".into()).unwrap(),
            "tata".into(),
        )
        .unwrap();
        let sourced =
            conduit_speech::contextual_intent_realization::prepare_sourced_contextual_intent(
                &intent,
                &[conduit_speech::intent_sources::IntentSourceMaterial::Text(&text); 4],
                &inventory,
                &profile,
                &boundaries,
                &policy,
                &contexts,
            )
            .map_err(|e| format!("{e:?}"))?;
        super::write_rendered(
            output,
            &format!("sourced-contextual-{name}"),
            sourced.renderer().map_err(|e| format!("{e:?}"))?,
        )?;
        println!(
            "{name}: {} exact original source references retained",
            sourced.sources().receipts().len()
        );
    }
    global_rule::write(output, &intent, &inventory, &profile)?;
    Ok(())
}
