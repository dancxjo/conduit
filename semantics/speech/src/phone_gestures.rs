//! Finite G2 preparation from one opaque G1 owner. No PCM or detached phone tape.
//!
//! Hosted preparation admits at most256 original events,64 authored definition
//! controls,1280 trajectories and1792 retained Source executions. Each execution
//! retains at most64KiB input and64KiB output; Generic Audio separately admits
//! its existing bounded trajectory/Source traces. Profile lookup and neighbor
//! bookkeeping are bounded traversals, not phonological or numerical policies.
//! All cumulative timing, overlap and energy phase construction is checked
//! Source. Original semantic duration fractions survive; 8kHz VoiceEvent/frame
//! spans are never timing inputs. Preparation may allocate; this is not an
//! allocation-free Play Back, installed speaking grant or naturalness claim.
use crate::{contextual_intent_realization::PreparedContextualIntent, semantic::*};
use alloc::vec::Vec;
use conduit_audio::{
    AudioLinearTrajectoryProfile, AudioQuantityTrajectory, AudioTrajectoryAnchor,
    AudioTrajectoryProvenance, AudioTrajectoryQuantity, PreparedAudioQuantityTrajectory,
};
use conduit_plot::rust_binding::NativeRustBinding;
#[path = "phone_gestures/prepare.rs"]
mod prepare;
#[path = "phone_gestures/source.rs"]
mod source;
pub use prepare::prepare_phone_gestures;
pub use source::GestureSourceExecution;

#[derive(Debug)]
pub enum PhoneGestureRefusal {
    Capacity,
    ForeignOwner,
    ForeignDefinition,
    DuplicateControl,
    MissingControl,
    InvalidDuration { event: usize },
    Source,
    Evaluation(conduit_plot::PortableExpressionEvaluationRefusal),
    Domain,
    Native(conduit_plot::rust_binding::NativeBindingRefusal),
    Audio(conduit_audio::AudioTrajectoryRefusal),
}
/// Explicit independent semantic quantities; these are not legacy voice presets.
#[derive(Clone)]
pub struct PhoneGestureControl<'a> {
    pub definition: &'a SpeechPhone,
    pub resonance: [AudioTrajectoryQuantity; 3],
    pub energy: AudioTrajectoryQuantity,
    pub closure: AudioTrajectoryQuantity,
    pub release: AudioTrajectoryQuantity,
    pub aspiration: AudioTrajectoryQuantity,
}
/// Exact selected definition membership belongs to this original G1 owner.
pub struct PhoneGestureProfile<'a, 'intent> {
    owner: &'a PreparedContextualIntent<'intent>,
    identity: &'a str,
    controls: &'a [PhoneGestureControl<'intent>],
}
impl<'a, 'intent> PhoneGestureProfile<'a, 'intent> {
    pub fn prepare(
        owner: &'a PreparedContextualIntent<'intent>,
        identity: &'a str,
        controls: &'a [PhoneGestureControl<'intent>],
    ) -> Result<Self, PhoneGestureRefusal> {
        if identity.is_empty() || identity.len() > 128 || controls.len() > 64 {
            return Err(PhoneGestureRefusal::Capacity);
        }
        for (n, c) in controls.iter().enumerate() {
            if !owner
                .phones()
                .iter()
                .any(|p| core::ptr::eq(p.definition(), c.definition))
            {
                return Err(PhoneGestureRefusal::ForeignDefinition);
            }
            if controls[..n]
                .iter()
                .any(|p| p.definition.identity() == c.definition.identity())
            {
                return Err(PhoneGestureRefusal::DuplicateControl);
            }
            if !c
                .resonance
                .iter()
                .all(|q| matches!(q, AudioTrajectoryQuantity::Frequency(_)))
                || ![&c.energy, &c.closure, &c.release, &c.aspiration]
                    .iter()
                    .all(|q| matches!(q, AudioTrajectoryQuantity::Amplitude(_)))
            {
                return Err(PhoneGestureRefusal::Domain);
            }
        }
        if owner.phones().iter().any(|p| {
            !controls
                .iter()
                .any(|c| core::ptr::eq(p.definition(), c.definition))
        }) {
            return Err(PhoneGestureRefusal::MissingControl);
        }
        Ok(Self {
            owner,
            identity,
            controls,
        })
    }
    pub fn controls(&self) -> &[PhoneGestureControl<'intent>] {
        self.controls
    }
    pub fn identity(&self) -> &str {
        self.identity
    }
    pub fn owner(&self) -> &'a PreparedContextualIntent<'intent> {
        self.owner
    }
    fn control(
        &self,
        segment: usize,
    ) -> Result<&PhoneGestureControl<'intent>, PhoneGestureRefusal> {
        let definition = self
            .owner
            .phones()
            .get(segment)
            .ok_or(PhoneGestureRefusal::ForeignDefinition)?
            .definition();
        self.controls
            .iter()
            .find(|c| core::ptr::eq(c.definition, definition))
            .ok_or(PhoneGestureRefusal::MissingControl)
    }
}
#[derive(Clone, Copy)]
pub struct PhoneGestureCapacity {
    pub events: usize,
    pub trajectories: usize,
    pub source_executions: usize,
}
/// Each current phone retains five trajectories and their original Source bytes.
pub struct PhoneGestures {
    pub event: usize,
    pub segment: usize,
    pub previous_segment: Option<usize>,
    pub next_segment: Option<usize>,
    pub timing: SpeechPhoneGestureTiming,
    pub phases: SpeechPhoneGesturePhases,
    resonance: [PreparedAudioQuantityTrajectory; 3],
    energy: PreparedAudioQuantityTrajectory,
    phases_energy: PreparedAudioQuantityTrajectory,
    phase_source: conduit_audio::AudioQuantityTrajectory,
}
impl PhoneGestures {
    pub fn resonance(&self) -> &[PreparedAudioQuantityTrajectory; 3] {
        &self.resonance
    }
    pub fn energy(&self) -> &PreparedAudioQuantityTrajectory {
        &self.energy
    }
    pub fn closure_release_aspiration(&self) -> &PreparedAudioQuantityTrajectory {
        &self.phases_energy
    }
    /// Source-authored phase order; each interval and value remains inspectable.
    pub fn closure(&self) -> &conduit_audio::AudioTrajectorySegment {
        &self.phase_source.segments().as_slice()[0]
    }
    pub fn release(&self) -> &conduit_audio::AudioTrajectorySegment {
        &self.phase_source.segments().as_slice()[1]
    }
    pub fn aspiration(&self) -> &conduit_audio::AudioTrajectorySegment {
        &self.phase_source.segments().as_slice()[2]
    }
}
pub struct PreparedPhoneGestures<'a, 'intent> {
    original: &'a PreparedContextualIntent<'intent>,
    profile: &'a PhoneGestureProfile<'a, 'intent>,
    anchor: AudioTrajectoryAnchor,
    phones: Vec<PhoneGestures>,
    executions: Vec<GestureSourceExecution>,
}
impl<'a, 'intent> PreparedPhoneGestures<'a, 'intent> {
    pub fn original(&self) -> &'a PreparedContextualIntent<'intent> {
        self.original
    }
    pub fn profile(&self) -> &PhoneGestureProfile<'a, 'intent> {
        self.profile
    }
    pub fn anchor(&self) -> &AudioTrajectoryAnchor {
        &self.anchor
    }
    pub fn phones(&self) -> &[PhoneGestures] {
        &self.phones
    }
    pub fn executions(&self) -> &[GestureSourceExecution] {
        &self.executions
    }
    pub fn selected(
        &self,
        gesture: &PhoneGestures,
    ) -> Option<&crate::contextual_intent_realization::ContextualPhoneReceipt<'intent>> {
        self.phones
            .iter()
            .any(|p| core::ptr::eq(p, gesture))
            .then(|| &self.original.phones()[gesture.segment])
    }
}
fn admit_audio(
    curve: AudioQuantityTrajectory,
) -> Result<PreparedAudioQuantityTrajectory, PhoneGestureRefusal> {
    PreparedAudioQuantityTrajectory::new_with_linear_profile(
        &curve.encode().map_err(PhoneGestureRefusal::Native)?,
        AudioLinearTrajectoryProfile::CommonBasisU32,
    )
    .map_err(PhoneGestureRefusal::Audio)
}
