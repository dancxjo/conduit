//! Mechanical composition records original word occurrences and exact composite
//! occurrences explicitly. Phonetics, stress and source refs are copied intact.
use super::language;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{
    lexical_pronunciation::*, linguistic_prosody::*, pitch_trajectory::*, pronunciation_intent::*,
    semantic::*,
};
pub struct Composite {
    pub source: SpeechUtteranceIntent,
    pub words: Vec<SpeechUtteranceIntent>,
    pub correspondence: Vec<(LanguageSpeechTokenRef, LanguageSpeechTokenRef)>,
    pub segments: Vec<SpeechPlannedSegmentIntent>,
    pub bindings: Vec<SpeechLinguisticProsodyBinding>,
    pub trajectories: Vec<SpeechLinearPitchTrajectory>,
    pub events: Vec<usize>,
    pub word_indices: Vec<usize>,
}
pub fn compose(case: &language::Case, pronunciations: &[PreparedPronunciation<'_>]) -> Composite {
    compose_with_duration(
        case,
        pronunciations,
        SpeechDurationSpecification::known(10, 1).unwrap(),
    )
}
/// The caller selects duration; existing Source prosody and realization contracts
/// admit it and preserve the resulting exact common intent for both backends.
pub fn compose_with_duration(
    case: &language::Case,
    pronunciations: &[PreparedPronunciation<'_>],
    duration: SpeechDurationSpecification,
) -> Composite {
    let material = case.lexical.tape().source().material();
    let revision_identity = format!("phones/{}", material.revision().get());
    let revision_identity = if revision_identity.len() <= 64 {
        revision_identity
    } else {
        material.revision().get().to_owned()
    };
    let revision = SpeechSegmentRevisionId::new(revision_identity).unwrap();
    let utterance = SpeechUtteranceId::new("utterance".into()).unwrap();
    let sequence = SpeechSegmentSequenceId::new("proof/composite/phones".into()).unwrap();
    let mut source_events = Vec::new();
    let mut words = Vec::new();
    let mut correspondence = Vec::new();
    let mut segments = Vec::new();
    let mut bindings = Vec::new();
    let mut trajectories = Vec::new();
    let mut events = Vec::new();
    let mut word_indices = Vec::new();
    for (word, pronunciation) in pronunciations.iter().enumerate() {
        let rich = case.spoken_ordinals[word] == case.vocative;
        let basis = if rich {
            LinguisticProsodyBasis::Rich(&case.rich)
        } else {
            LinguisticProsodyBasis::Fallback(&case.fallback[word])
        };
        let prosody = SpeechSegmentProsodyIntent::new(
            duration.clone(),
            SpeechCycleSpecification::known(100, 1).unwrap(),
            SpeechIntensitySpecification::known(if rich { 4 } else { 2 }, if rich { 3 } else { 1 })
                .unwrap(),
        )
        .unwrap();
        let origin = LanguageSpeechTokenRef::new(
            case.inventory.identity().clone(),
            material.language().clone(),
            0,
            revision.clone(),
            SpeechSegmentSequenceId::new(format!("proof/word/{word}")).unwrap(),
            SpeechUtteranceId::new(format!("proof/word-utterance/{word}")).unwrap(),
        )
        .unwrap();
        let word_intent = prepare_pronunciation_intent(
            pronunciation,
            &origin,
            &prosody,
            &language::speech_provenance(),
        )
        .unwrap();
        for event in word_intent.intent().events().iter() {
            let SpeechUtteranceIntentEvent::Segment(old) = event else {
                panic!("one checked word sequence")
            };
            let current = LanguageSpeechTokenRef::new(
                case.inventory.identity().clone(),
                material.language().clone(),
                segments.len() as u32,
                revision.clone(),
                sequence.clone(),
                utterance.clone(),
            )
            .unwrap();
            correspondence.push((old.occurrence().clone(), current.clone()));
            let segment = SpeechPlannedSegmentIntent::new(
                current,
                old.phone().clone(),
                old.phoneme().clone(),
                old.prosody().clone(),
                old.provenance().clone(),
                old.sources().clone(),
                old.stress().clone(),
                old.word_position().clone(),
            )
            .unwrap();
            let binding = SpeechLinguisticProsodyBinding::new(
                basis.choice().clone(),
                case.voice.identity().clone(),
                material.language().clone(),
                basis.profile().identity().clone(),
                language::speech_provenance(),
                SpeechLinguisticProsodyRealization::new(
                    SpeechDurationSpecification::known(100, if rich { 3 } else { 0 }).unwrap(),
                    SpeechBoundarySpecification::Known(if rich {
                        SpeechBoundaryKind::Phrase
                    } else {
                        SpeechBoundaryKind::Word
                    }),
                    segment.prosody().clone(),
                )
                .unwrap(),
            )
            .unwrap();
            let SpeechDurationSpecification::Known(selected_duration) =
                segment.prosody().duration()
            else {
                panic!("explicit admitted duration")
            };
            let trajectory = SpeechLinearPitchTrajectory::new(
                SpeechExactDuration::new(
                    *selected_duration.denominator(),
                    *selected_duration.numerator_seconds(),
                )
                .unwrap(),
                SpeechFundamentalCycle::new(if rich { 150 } else { 100 }, 1).unwrap(),
                SpeechFundamentalCycle::new(100, 1).unwrap(),
            )
            .unwrap();
            events.push(source_events.len());
            word_indices.push(word);
            bindings.push(binding);
            trajectories.push(trajectory);
            source_events.push(
                SpeechUtteranceIntentEvent::segment(
                    segment.occurrence().clone(),
                    segment.phone().clone(),
                    segment.phoneme().clone(),
                    segment.prosody().clone(),
                    segment.provenance().clone(),
                    segment.sources().clone(),
                    segment.stress().clone(),
                    segment.word_position().clone(),
                )
                .unwrap(),
            );
            segments.push(segment);
        }
        if rich {
            let SpeechUtteranceIntentEvent::Segment(last) =
                word_intent.intent().events().iter().last().unwrap()
            else {
                panic!("word phones")
            };
            source_events.push(
                SpeechUtteranceIntentEvent::boundary(
                    SpeechDurationSpecification::known(100, 3).unwrap(),
                    SpeechBoundarySpecification::Known(SpeechBoundaryKind::Phrase),
                    language::speech_provenance(),
                    last.sources().clone(),
                )
                .unwrap(),
            );
        }
        words.push(word_intent.intent().clone());
    }
    let source = SpeechUtteranceIntent::new(
        BoundedSequence::try_from_iter(source_events).unwrap(),
        case.inventory.identity().clone(),
        material.language().clone(),
        language::speech_provenance(),
        revision,
        utterance,
    )
    .unwrap();
    Composite {
        source,
        words,
        correspondence,
        segments,
        bindings,
        trajectories,
        events,
        word_indices,
    }
}
impl Composite {
    pub fn linguistic<'a>(&'a self, case: &'a language::Case) -> Vec<PreparedLinguisticPitch<'a>> {
        self.segments
            .iter()
            .enumerate()
            .map(|(index, segment)| {
                let word = self.word_indices[index];
                let basis = if case.spoken_ordinals[word] == case.vocative {
                    LinguisticProsodyBasis::Rich(&case.rich)
                } else {
                    LinguisticProsodyBasis::Fallback(&case.fallback[word])
                };
                prepare_linguistic_pitch(
                    basis,
                    &self.bindings[index],
                    segment,
                    &self.trajectories[index],
                )
                .unwrap()
            })
            .collect()
    }
}
