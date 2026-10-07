#![allow(dead_code)]
use conduit_language::{pronunciation_selection::PreparedPronunciationSelection, *};
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{intent_realization::*, lexical_pronunciation::*, semantic::*};
pub fn profile() -> SpeechPronunciationProfile {
    let phone = |id: &str, stress| {
        SpeechPronunciationPhone::new(PhoneId::new(id.into()).unwrap(), stress).unwrap()
    };
    let candidate =
        |pos| LanguageLexicalCandidate::new("record".into(), BoundedSequence::new(), pos).unwrap();
    SpeechPronunciationProfile::new(
        "pronunciation/record-review".into(),
        LanguageId::new("language/en".into()).unwrap(),
        provenance(),
        BoundedSequence::try_from_iter([
            SpeechPronunciationRow::new(
                candidate(LanguageLexicalPos::Noun),
                BoundedSequence::try_from_iter([
                    phone("r", SpeechStress::Unstressed),
                    phone("eh", SpeechStress::Primary),
                    phone("k", SpeechStress::Unstressed),
                    phone("ax", SpeechStress::Unstressed),
                    phone("r", SpeechStress::Unstressed),
                    phone("d", SpeechStress::Unstressed),
                ])
                .unwrap(),
            )
            .unwrap(),
            SpeechPronunciationRow::new(
                candidate(LanguageLexicalPos::Verb),
                BoundedSequence::try_from_iter([
                    phone("r", SpeechStress::Unstressed),
                    phone("ih", SpeechStress::Unstressed),
                    phone("k", SpeechStress::Unstressed),
                    phone("ao", SpeechStress::Primary),
                    phone("r", SpeechStress::Unstressed),
                    phone("d", SpeechStress::Unstressed),
                ])
                .unwrap(),
            )
            .unwrap(),
        ])
        .unwrap(),
    )
    .unwrap()
}
fn provenance() -> SpeechEvidenceProvenance {
    SpeechEvidenceProvenance::new(
        "reviewed supplied phonetics".into(),
        SpeechEvidenceSource::Manual,
        None,
    )
    .unwrap()
}
pub struct Fixture {
    pub intent: SpeechUtteranceIntent,
    pub inventory: SpeechInventory,
    pub voice: SpeechFormantVoiceProfile,
    pub boundaries: SpeechFormantBoundaryProfile,
}
pub fn fixture(
    selection: &PreparedPronunciationSelection,
    pronunciation: &PreparedPronunciation<'_>,
) -> Fixture {
    use conduit_speech::semantic::EnglishPhone as E;
    let language = selection.request().source().material().language().clone();
    let inventory_id = SpeechInventoryId::new("reviewed/record-inventory".into()).unwrap();
    let origin = LanguageSpeechTokenRef::new(
        inventory_id.clone(),
        language.clone(),
        0,
        SpeechSegmentRevisionId::new("phones/0".into()).unwrap(),
        SpeechSegmentSequenceId::new("record/phones".into()).unwrap(),
        SpeechUtteranceId::new(format!(
            "utterance/{}",
            selection.request().source().material().identity().get()
        ))
        .unwrap(),
    )
    .unwrap();
    let prosody = SpeechSegmentProsodyIntent::new(
        SpeechDurationSpecification::known(10, 1).unwrap(),
        SpeechCycleSpecification::known(100, 1).unwrap(),
        SpeechIntensitySpecification::known(1, 1).unwrap(),
    )
    .unwrap();
    let prepared = conduit_speech::pronunciation_intent::prepare_pronunciation_intent(
        pronunciation,
        &origin,
        &prosody,
        &provenance(),
    )
    .unwrap();
    let intent = prepared.intent().clone();
    // Explicit reviewed inventory rows: these are supplied data, not language
    // selection policy hidden in a renderer or production Rust frontend.
    let rows = [
        ("r", "ɹ", E::R),
        ("eh", "ɛ", E::Eh),
        ("k", "k", E::K),
        ("ax", "ə", E::Ax),
        ("d", "d", E::D),
        ("ih", "ɪ", E::Ih),
        ("ao", "ɔ", E::Ao),
    ];
    let definitions = rows
        .iter()
        .map(|(id, ipa, _)| {
            SpeechPhone::new(
                BoundedSequence::new(),
                SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
                PhoneId::new((*id).into()).unwrap(),
                (*ipa).into(),
                SpeechSegmentStatus::Core,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let bindings = definitions
        .iter()
        .zip(rows)
        .map(|(definition, (_, _, profile))| {
            SpeechFormantPhoneBinding::new(definition.clone(), profile).unwrap()
        });
    let voice = SpeechFormantVoiceProfile::new(
        "reviewed/record-voice".into(),
        inventory_id.clone(),
        language.clone(),
        BoundedSequence::try_from_iter(bindings).unwrap(),
    )
    .unwrap();
    let inventory = SpeechInventory::new(
        inventory_id,
        language,
        BoundedSequence::new(),
        BoundedSequence::try_from_iter(definitions).unwrap(),
    )
    .unwrap();
    Fixture {
        intent,
        inventory,
        voice,
        boundaries: SpeechFormantBoundaryProfile::new(BoundedSequence::new()).unwrap(),
    }
}
impl Fixture {
    pub fn realize(&self) -> PreparedIntentRealization<'_> {
        prepare_intent_realization(&self.intent, &self.inventory, &self.voice, &self.boundaries)
            .unwrap()
    }
}
