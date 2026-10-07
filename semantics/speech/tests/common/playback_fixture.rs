//! Explicit supplied UD/prosody/phone data; no parser or pronunciation claim.
#[path = "../../../language/tests/common/prosody.rs"]
#[allow(dead_code)]
mod language_fixture;
use conduit_language::{prosody::*, *};
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{
    intent_realization::*, linguistic_prosody::*, pitch_trajectory::*, semantic::*,
};
pub struct Fixture {
    pub rich: PreparedRichProsody,
    pub binding: SpeechLinguisticProsodyBinding,
    pub segment: SpeechPlannedSegmentIntent,
    pub trajectory: SpeechLinearPitchTrajectory,
    pub source: SpeechUtteranceIntent,
    pub inventory: SpeechInventory,
    pub voice: SpeechFormantVoiceProfile,
    pub boundaries: SpeechFormantBoundaryProfile,
}
fn provenance() -> SpeechEvidenceProvenance {
    SpeechEvidenceProvenance::new(
        "reviewed fixture".into(),
        SpeechEvidenceSource::Manual,
        None,
    )
    .unwrap()
}
pub fn fixture(text: &str, revision: &str) -> Fixture {
    let language = LanguageId::new("language/en".into()).unwrap();
    let old = language_fixture::fixture(text, 1, 0);
    let material = LanguageText::new(
        LanguageTextId::new("text".into()).unwrap(),
        language.clone(),
        LanguageTextRevisionId::new(revision.into()).unwrap(),
        text.into(),
    )
    .unwrap();
    let source_revision = LanguageTextRevision::new(
        LanguageTextFinality::Final,
        material,
        None,
        language_fixture::provenance(),
        0,
        None,
    )
    .unwrap();
    let lexical = conduit_language::lexical::prepare_lexical_tape(
        &source_revision,
        old.lexical.tape().profile(),
        None,
    )
    .unwrap();
    let analysis = LanguageAnalysisRevisionId::new(format!("analysis/{revision}")).unwrap();
    let token = |ordinal| {
        LinguisticTokenIdentity::new(
            ordinal,
            source_revision.material().identity().clone(),
            source_revision.material().revision().clone(),
        )
        .unwrap()
    };
    let arc = LanguageDependencyArc::new(
        LanguageAnalysisTokenRef::new(analysis.clone(), token(1)).unwrap(),
        LanguageDependencyHead::token(analysis.clone(), token(0)).unwrap(),
        LanguageDependencyRelation::new(LanguageUniversalDependencyRelation::Vocative, None)
            .unwrap(),
    )
    .unwrap();
    let discourse = conduit_language::discourse::prepare_vocative_fact(
        format!("fact/{revision}"),
        &source_revision,
        &analysis,
        &arc,
        language_fixture::provenance(),
        2,
    )
    .unwrap();
    let rich = prepare_rich_prosody(&lexical, 1, discourse.fact(), &old.profile).unwrap();
    let reference = conduit_language::language_source_occurrence(
        rich.requested().source().material(),
        rich.requested().token().span(),
        LanguageTextSegmentKind::Word,
    )
    .unwrap();
    let segment = SpeechPlannedSegmentIntent::new(
        LanguageSpeechTokenRef::new(
            SpeechInventoryId::new("inv".into()).unwrap(),
            language.clone(),
            0,
            SpeechSegmentRevisionId::new(revision.into()).unwrap(),
            SpeechSegmentSequenceId::new(format!("phones/{revision}")).unwrap(),
            SpeechUtteranceId::new("utterance".into()).unwrap(),
        )
        .unwrap(),
        PhoneSpecification::known(PhoneId::new("vowel".into()).unwrap()).unwrap(),
        PhonemeSpecification::unspecified(),
        SpeechSegmentProsodyIntent::new(
            SpeechDurationSpecification::known(5, 1).unwrap(),
            SpeechCycleSpecification::known(100, 1).unwrap(),
            SpeechIntensitySpecification::known(1, 1).unwrap(),
        )
        .unwrap(),
        provenance(),
        BoundedSequence::try_from_iter([LanguageSegmentRef::text(
            *reference.kind(),
            reference.language().clone(),
            reference.range().clone(),
            reference.revision_id().clone(),
            reference.text_id().clone(),
        )
        .unwrap()])
        .unwrap(),
        StressSpecification::unknown(),
        SpeechPositionSpecification::unspecified(),
    )
    .unwrap();
    let binding = SpeechLinguisticProsodyBinding::new(
        rich.accepted().choice().clone(),
        "voice".into(),
        language.clone(),
        old.profile.identity().clone(),
        provenance(),
        SpeechLinguisticProsodyRealization::new(
            SpeechDurationSpecification::known(100, 3).unwrap(),
            SpeechBoundarySpecification::Known(SpeechBoundaryKind::Phrase),
            segment.prosody().clone(),
        )
        .unwrap(),
    )
    .unwrap();
    let trajectory = SpeechLinearPitchTrajectory::new(
        SpeechExactDuration::new(5, 1).unwrap(),
        SpeechFundamentalCycle::new(100, 1).unwrap(),
        SpeechFundamentalCycle::new(100, 1).unwrap(),
    )
    .unwrap();
    let event = SpeechUtteranceIntentEvent::segment(
        segment.occurrence().clone(),
        segment.phone().clone(),
        segment.phoneme().clone(),
        segment.prosody().clone(),
        segment.provenance().clone(),
        segment.sources().clone(),
        segment.stress().clone(),
        segment.word_position().clone(),
    )
    .unwrap();
    let source = SpeechUtteranceIntent::new(
        BoundedSequence::try_from_iter([event]).unwrap(),
        segment.occurrence().inventory_id().clone(),
        language.clone(),
        provenance(),
        segment.occurrence().revision_id().clone(),
        segment.occurrence().utterance_id().clone(),
    )
    .unwrap();
    let definition = SpeechPhone::new(
        BoundedSequence::new(),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        PhoneId::new("vowel".into()).unwrap(),
        "ɑ".into(),
        SpeechSegmentStatus::Allophonic,
    )
    .unwrap();
    let voice = SpeechFormantVoiceProfile::new(
        "voice".into(),
        SpeechInventoryId::new("inv".into()).unwrap(),
        language.clone(),
        BoundedSequence::try_from_iter([SpeechFormantPhoneBinding::new(
            definition.clone(),
            EnglishPhone::Aa,
        )
        .unwrap()])
        .unwrap(),
    )
    .unwrap();
    let inventory = SpeechInventory::new(
        SpeechInventoryId::new("inv".into()).unwrap(),
        language,
        BoundedSequence::new(),
        BoundedSequence::try_from_iter([definition]).unwrap(),
    )
    .unwrap();
    let boundaries = SpeechFormantBoundaryProfile::new(BoundedSequence::new()).unwrap();
    Fixture {
        rich,
        binding,
        segment,
        trajectory,
        source,
        inventory,
        voice,
        boundaries,
    }
}
impl Fixture {
    pub fn linguistic(&self) -> PreparedLinguisticPitch<'_> {
        prepare_linguistic_pitch(
            LinguisticProsodyBasis::Rich(&self.rich),
            &self.binding,
            &self.segment,
            &self.trajectory,
        )
        .unwrap()
    }
    pub fn realized(&self) -> PreparedIntentRealization<'_> {
        prepare_intent_realization(&self.source, &self.inventory, &self.voice, &self.boundaries)
            .unwrap()
    }
}
