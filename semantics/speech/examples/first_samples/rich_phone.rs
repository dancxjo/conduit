//! Listening fixture: explicit inventory/profile declarations, not an IPA parser.
use conduit_core::IeeeF32;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{
    inventory_admission::resolve_inventory_phone, profile_admission::prepare_profile_phone,
    reference_admission::resolve_phone, semantic::*, EnglishPhone as C, EnglishStress as S,
    Renderer, VoiceBoundary, VoiceEvent, MAXIMUM_EVENTS,
};
fn fixture_phone(phone: C) -> (&'static str, EnglishPhone) {
    match phone {
        C::h => ("h", EnglishPhone::H),
        C::ax => ("ə", EnglishPhone::Ax),
        C::eh => ("ɛ", EnglishPhone::Eh),
        C::l => ("l", EnglishPhone::L),
        C::ow => ("oʊ", EnglishPhone::Ow),
        C::w => ("w", EnglishPhone::W),
        C::er => ("ɝ", EnglishPhone::Er),
        C::r => ("ɹ", EnglishPhone::R),
        C::d => ("d", EnglishPhone::D),
        _ => panic!("fixture does not declare this phone"),
    }
}
fn stress(value: S) -> StressSpecification {
    match value {
        S::primary => StressSpecification::known(SpeechStress::Primary).unwrap(),
        S::secondary => StressSpecification::known(SpeechStress::Secondary).unwrap(),
        S::unstressed => StressSpecification::known(SpeechStress::Unstressed).unwrap(),
        S::reduced => StressSpecification::known(SpeechStress::Reduced).unwrap(),
        S::unknown => StressSpecification::unknown(),
        S::unspecified => StressSpecification::unspecified(),
    }
}
pub fn write(output: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut storage = [VoiceEvent::boundary(VoiceBoundary::word); MAXIMUM_EVENTS];
    let pronounced =
        conduit_speech::pronounce("Hello, world!", &mut storage).map_err(|e| format!("{e:?}"))?;
    let inventory_id = SpeechInventoryId::new("fixture/inventory".into()).unwrap();
    let language = SpeechLanguageId::new("en".into()).unwrap();
    let revision = SpeechSegmentRevisionId::new("fixture/revision".into()).unwrap();
    let sequence = SpeechSegmentSequenceId::new("fixture/phones".into()).unwrap();
    let utterance = SpeechUtteranceId::new("fixture/hello".into()).unwrap();
    let mut definitions = Vec::new();
    let mut bindings = Vec::new();
    let mut tokens = Vec::new();
    let mut references = Vec::new();
    let mut stresses = Vec::new();
    for input in pronounced
        .events()
        .iter()
        .filter_map(|event| event.realization())
    {
        let selected = conduit_speech::realize(input).unwrap();
        let ordinal = u32::try_from(tokens.len())?;
        let id = PhoneId::new(format!("fixture/phone/{ordinal}")).unwrap();
        let (ipa, profile_phone) = fixture_phone(selected.phone);
        let definition = SpeechPhone::new(
            BoundedSequence::new(),
            SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
            id.clone(),
            ipa.into(),
            SpeechSegmentStatus::Core,
        )
        .unwrap();
        bindings.push(SpeechFormantPhoneBinding::new(definition.clone(), profile_phone).unwrap());
        definitions.push(definition);
        tokens.push(
            SpeechPhoneToken::new(
                BoundedSequence::new(),
                SpeechConfidence::new(IeeeF32::from_value(1.0)).unwrap(),
                SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
                PhoneSpecification::known(id).unwrap(),
                SpeechEvidenceProvenance::new(
                    "listening fixture".into(),
                    SpeechEvidenceSource::Manual,
                    None,
                )
                .unwrap(),
                None,
            )
            .unwrap(),
        );
        references.push(
            LanguageSegmentRef::phone(
                inventory_id.clone(),
                language.clone(),
                ordinal,
                revision.clone(),
                sequence.clone(),
                utterance.clone(),
            )
            .unwrap(),
        );
        stresses.push(stress(input.stress));
    }
    let snapshot = SpeechPhoneSequence::new(
        SpeechTokenSequenceBasis::new(
            inventory_id.clone(),
            language.clone(),
            revision,
            sequence,
            utterance,
        )
        .unwrap(),
        BoundedSequence::try_from_iter(tokens).unwrap(),
    )
    .unwrap();
    let inventory = SpeechInventory::new(
        inventory_id.clone(),
        language.clone(),
        BoundedSequence::new(),
        BoundedSequence::try_from_iter(definitions).unwrap(),
    )
    .unwrap();
    let profile = SpeechFormantVoiceProfile::new(
        "fixture/formant".into(),
        inventory_id,
        language,
        BoundedSequence::try_from_iter(bindings).unwrap(),
    )
    .unwrap();
    let materials = references
        .iter()
        .map(|reference| resolve_phone(reference, &snapshot).unwrap())
        .collect::<Vec<_>>();
    let inventory_phones = materials
        .iter()
        .map(|material| resolve_inventory_phone(material, &inventory).unwrap())
        .collect::<Vec<_>>();
    let prepared = inventory_phones
        .iter()
        .zip(&stresses)
        .map(|(phone, stress)| prepare_profile_phone(phone, &profile, stress).unwrap())
        .collect::<Vec<_>>();
    let mut phones = prepared.iter();
    let events = pronounced
        .events()
        .iter()
        .map(|event| {
            if event.realization().is_some() {
                phones.next().unwrap().event()
            } else {
                *event
            }
        })
        .collect::<Vec<_>>();
    // All source snapshots and preparation receipts remain alive for this render.
    super::write_rendered(
        output,
        "rich-phone-hello-world",
        Renderer::prepare(&events).map_err(|e| format!("{e:?}"))?,
    )?;
    println!("{output}/rich-phone-hello-world.wav: explicit rich definitions and native profile eligibility; source {}", prepared[0].compiled_source_id());
    Ok(())
}
