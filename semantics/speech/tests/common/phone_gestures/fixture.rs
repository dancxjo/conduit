//! Explicit G2 controls borrow the approved G1 specimen, not renderer constants.
#[allow(dead_code)] // Reuse the approved declaration/tape without duplicating its unrelated exports.
#[path = "../contextual_ipa_profile/carrier.rs"]
mod carrier;
#[allow(dead_code)] // Reuse the approved declaration/tape without duplicating its unrelated exports.
#[path = "../contextual_ipa_profile/declaration.rs"]
pub mod declaration;
use conduit_audio::*;
use conduit_speech::{
    contextual_intent_realization::*, declared_context::ExplicitAllophoneContext,
    phone_gestures::*, semantic::*,
};
pub struct Fixture {
    pub intent: SpeechUtteranceIntent,
    pub inventory: SpeechInventory,
    pub voice: SpeechFormantVoiceProfile,
    boundaries: SpeechFormantBoundaryProfile,
    policy: SpeechAllophoneChoicePolicy,
    style: SpeechCarefulStyleSpecification,
    syllable: SpeechSyllablePositionSpecification,
    prosody: SpeechProsodicContextSpecification,
}
impl Fixture {
    pub fn new(boundary: bool) -> Self {
        let inventory = declaration::inventory(false);
        let voice = declaration::formant(&inventory);
        let original = carrier::intent();
        let mut events = original.events().as_slice().to_vec();
        if boundary {
            let SpeechUtteranceIntentEvent::Segment(previous) = &events[3] else {
                panic!("segment")
            };
            let sources = previous.sources().clone();
            events.insert(
                4,
                SpeechUtteranceIntentEvent::boundary(
                    SpeechDurationSpecification::known(30, 1).unwrap(),
                    SpeechBoundarySpecification::known(SpeechBoundaryKind::Word).unwrap(),
                    declaration::evidence(),
                    sources,
                )
                .unwrap(),
            );
        }
        let intent = SpeechUtteranceIntent::new(
            declaration::seq(events),
            original.inventory_id().clone(),
            original.language().clone(),
            original.provenance().clone(),
            original.revision_id().clone(),
            original.utterance_id().clone(),
        )
        .unwrap();
        Self {
            intent,
            voice,
            inventory,
            boundaries: SpeechFormantBoundaryProfile::new(declaration::seq([
                SpeechFormantBoundaryBinding::new(
                    SpeechBoundaryKind::Word,
                    SpeechFormantBoundary::Word,
                )
                .unwrap(),
            ]))
            .unwrap(),
            policy: SpeechAllophoneChoicePolicy::new(true, false, false, false, true, false)
                .unwrap(),
            style: SpeechCarefulStyleSpecification::known(false).unwrap(),
            syllable: SpeechSyllablePositionSpecification::unknown(),
            prosody: SpeechProsodicContextSpecification::unknown(),
        }
    }
    pub fn replace_duration(&mut self, event: usize, denominator: u64, numerator: u64) {
        let mut events = self.intent.events().as_slice().to_vec();
        let SpeechUtteranceIntentEvent::Segment(s) = &events[event] else {
            panic!("segment")
        };
        events[event] = SpeechUtteranceIntentEvent::segment(
            s.occurrence().clone(),
            s.phone().clone(),
            s.phoneme().clone(),
            SpeechSegmentProsodyIntent::new(
                SpeechDurationSpecification::known(denominator, numerator).unwrap(),
                s.prosody().fundamental_cycle().clone(),
                s.prosody().relative_intensity().clone(),
            )
            .unwrap(),
            s.provenance().clone(),
            s.sources().clone(),
            s.stress().clone(),
            s.word_position().clone(),
        )
        .unwrap();
        self.intent = SpeechUtteranceIntent::new(
            declaration::seq(events),
            self.intent.inventory_id().clone(),
            self.intent.language().clone(),
            self.intent.provenance().clone(),
            self.intent.revision_id().clone(),
            self.intent.utterance_id().clone(),
        )
        .unwrap();
    }
    pub fn owner(&self) -> PreparedContextualIntent<'_> {
        let contexts = self
            .intent
            .events()
            .as_slice()
            .iter()
            .map(|e| match e {
                SpeechUtteranceIntentEvent::Boundary(_) => None,
                _ => Some(ExplicitAllophoneContext {
                    careful_style: &self.style,
                    syllable_position: &self.syllable,
                    prosodic_context: &self.prosody,
                }),
            })
            .collect::<Vec<_>>();
        prepare_contextual_intent(
            &self.intent,
            &self.inventory,
            &self.voice,
            &self.boundaries,
            &self.policy,
            &contexts,
        )
        .unwrap()
    }
}
pub fn controls<'a>(owner: &PreparedContextualIntent<'a>) -> Vec<PhoneGestureControl<'a>> {
    // Authored G2@1 quantities. G1 selected phone definitions supply custody only.
    let mut controls = Vec::new();
    for receipt in owner.phones() {
        if controls.iter().any(|c: &PhoneGestureControl<'_>| {
            c.definition.identity() == receipt.definition().identity()
        }) {
            continue;
        }
        let (frequencies, energy, closure, release, aspiration) =
            match receipt.definition().identity().get().as_str() {
                "g1/aspirated-t" => ([270, 2290, 3010], 2, 0, 1, 1),
                "g1/t" => ([300, 2100, 2900], 1, 0, 1, 0),
                "g1/a" => ([730, 1090, 2440], 4, 1, 1, 1),
                _ => panic!("declared profile has no selected phone"),
            };
        let amplitude = |n| AudioTrajectoryQuantity::amplitude(1, n).unwrap();
        controls.push(PhoneGestureControl {
            definition: receipt.definition(),
            resonance: frequencies.map(|n| AudioTrajectoryQuantity::frequency(1, n).unwrap()),
            energy: AudioTrajectoryQuantity::amplitude(4, energy).unwrap(),
            closure: amplitude(closure),
            release: amplitude(release),
            aspiration: amplitude(aspiration),
        });
    }
    controls
}
pub fn anchor() -> AudioTrajectoryAnchor {
    AudioTrajectoryAnchor::new(
        AudioOriginIdentity::new(1).unwrap(),
        AudioTimelineIdentity::new(1).unwrap(),
    )
    .unwrap()
}
pub fn provenance() -> AudioTrajectoryProvenance {
    AudioTrajectoryProvenance::new(
        AudioTrajectoryProvenanceKind::Authored,
        "G2/tata-control-profile".into(),
        Some("1".into()),
    )
    .unwrap()
}
pub fn capacity() -> PhoneGestureCapacity {
    PhoneGestureCapacity {
        events: 256,
        trajectories: 1280,
        source_executions: 1792,
    }
}
