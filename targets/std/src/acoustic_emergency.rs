//! Out-of-band acoustic emergency admission for the std Host.
//!
//! The ordinary keyword gear remains inert. This host-owned adapter is the
//! narrow composition point that can turn exact validated microphone frames
//! into a one-shot authority-reduction request.

use conduit_body::{
    AcousticDecision, AcousticObservation, BodyId, DurableEmergencyConfiguration,
    EmergencyAcousticAvailability, EmergencyControl, EmergencyOutcome, EmergencyPolicy,
    EmergencyRefusal, EmergencyRequest, EmergencySequenceMatcher, EmergencyTriggerClass,
    EMERGENCY_CONTROL_POLICY,
};
use conduit_core::{BaseLifecycle, BaseProviderEntry, BootId, HostId};
use conduit_emergency_keyword_spotter::{
    DetectorDecision, DetectorRefusal, EmergencyKeywordSpotter, KeywordTemplate, ValidatedPcmFrame,
    PROFILE_ID, SAMPLES_PER_FRAME, SAMPLE_RATE_HZ, TARGET_WORDS,
};

const FRAME_MILLISECONDS: u64 = (SAMPLES_PER_FRAME as u64 * 1_000) / SAMPLE_RATE_HZ as u64;
const MAX_PROVIDER_ID_BYTES: usize = 96;
pub const ALSA_MICROPHONE_BASE_KIND: &str = "std.base/alsa-microphone@1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcousticMicrophoneBasis {
    provider_id: String,
    generation: u64,
    physically_muted: bool,
}

impl AcousticMicrophoneBasis {
    /// Projects the exact current microphone provider from canonical registry
    /// truth. Availability is never reconstructed from a capability offer.
    pub fn from_registry_entry(
        provider: &BaseProviderEntry,
        physically_muted: bool,
    ) -> Result<Self, AcousticEmergencyRefusal> {
        if provider.mechanism_family.as_str() != ALSA_MICROPHONE_BASE_KIND
            || provider.lifecycle != BaseLifecycle::Ready
            || provider.provider_instance_id.as_str().is_empty()
            || provider.provider_instance_id.as_str().len() > MAX_PROVIDER_ID_BYTES
            || provider.provider_generation == 0
        {
            return Err(AcousticEmergencyRefusal::InvalidProvider);
        }
        if physically_muted {
            return Err(AcousticEmergencyRefusal::PhysicallyMuted);
        }
        Ok(Self {
            provider_id: provider.provider_instance_id.as_str().into(),
            generation: provider.provider_generation,
            physically_muted,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcousticEmergencyAvailability {
    Ready,
    PhysicallyMuted,
    ProviderUnavailable,
    ProviderReplaced,
    InputOverflow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcousticEmergencyRefusal {
    InvalidConfiguration,
    WrongDetector,
    WrongTemplates,
    InvalidProvider,
    PhysicallyMuted,
    ProviderUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcousticEmergencyDecision {
    Waiting,
    PhraseAdvanced,
    AuthorityReductionRequested(EmergencyOutcome),
    Unavailable(AcousticEmergencyAvailability),
    Refused(EmergencyRefusal),
    Suppressed,
}

/// Fixed-lifetime, one-shot acoustic authority path.
///
/// This type has no Form, Plan, Play, transcript, model, Wake, grant, or resume
/// input. Replacing the microphone provider requires constructing a fresh
/// adapter against current host truth.
pub struct AcousticEmergencyAdapter {
    body_id: BodyId,
    host_id: HostId,
    boot_id: BootId,
    provider_id: String,
    generation: u64,
    availability: AcousticEmergencyAvailability,
    detector: EmergencyKeywordSpotter,
    sequence: EmergencySequenceMatcher,
    control: EmergencyControl,
    triggered: bool,
}

impl AcousticEmergencyAdapter {
    #[allow(clippy::too_many_arguments)]
    pub fn admit(
        configuration: &DurableEmergencyConfiguration,
        microphone: AcousticMicrophoneBasis,
        body_id: BodyId,
        host_id: HostId,
        boot_id: BootId,
        policy: EmergencyPolicy,
        templates: [KeywordTemplate; TARGET_WORDS],
    ) -> Result<Self, AcousticEmergencyRefusal> {
        configuration
            .validate()
            .map_err(|_| AcousticEmergencyRefusal::InvalidConfiguration)?;
        if configuration.detector_version != PROFILE_ID {
            return Err(AcousticEmergencyRefusal::WrongDetector);
        }
        if configuration.acoustic_availability != EmergencyAcousticAvailability::Ready {
            return Err(match configuration.acoustic_availability {
                EmergencyAcousticAvailability::PhysicallyMuted => {
                    AcousticEmergencyRefusal::PhysicallyMuted
                }
                _ => AcousticEmergencyRefusal::ProviderUnavailable,
            });
        }
        if microphone.physically_muted {
            return Err(AcousticEmergencyRefusal::PhysicallyMuted);
        }
        let mut expected = configuration.key.word_ids;
        let mut actual = templates.map(|template| template.word_id);
        expected.sort_unstable();
        actual.sort_unstable();
        if actual != expected {
            return Err(AcousticEmergencyRefusal::WrongTemplates);
        }
        let detector = EmergencyKeywordSpotter::new(microphone.generation, templates)
            .map_err(|_| AcousticEmergencyRefusal::WrongTemplates)?;
        let sequence = EmergencySequenceMatcher::new(configuration.key.clone())
            .map_err(|_| AcousticEmergencyRefusal::InvalidConfiguration)?;
        let control =
            EmergencyControl::admit(body_id.clone(), host_id.clone(), boot_id.clone(), policy);
        Ok(Self {
            body_id,
            host_id,
            boot_id,
            provider_id: microphone.provider_id,
            generation: microphone.generation,
            availability: AcousticEmergencyAvailability::Ready,
            detector,
            sequence,
            control,
            triggered: false,
        })
    }

    pub fn availability(&self) -> AcousticEmergencyAvailability {
        self.availability
    }

    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    pub fn provider_generation(&self) -> u64 {
        self.generation
    }

    /// Consumes one frame only after the host has validated its exact provider.
    pub fn observe_validated_frame(
        &mut self,
        provider_id: &str,
        frame: ValidatedPcmFrame<'_>,
    ) -> AcousticEmergencyDecision {
        if self.triggered {
            return AcousticEmergencyDecision::Suppressed;
        }
        if self.availability != AcousticEmergencyAvailability::Ready {
            return AcousticEmergencyDecision::Unavailable(self.availability);
        }
        if provider_id != self.provider_id || frame.microphone_generation != self.generation {
            self.availability = AcousticEmergencyAvailability::ProviderReplaced;
            let _ = self.sequence.observe(AcousticObservation::StaleGeneration);
            return AcousticEmergencyDecision::Unavailable(self.availability);
        }
        match self.detector.observe(frame) {
            DetectorDecision::Accumulating | DetectorDecision::NoMatch => {
                AcousticEmergencyDecision::Waiting
            }
            DetectorDecision::Detected {
                word_id,
                window_end_sequence,
                ..
            } => {
                let Some(ended_at_millis) = window_end_sequence.checked_mul(FRAME_MILLISECONDS)
                else {
                    return self.input_overflow();
                };
                match self.sequence.observe(AcousticObservation::Word {
                    word_id,
                    ended_at_millis,
                }) {
                    AcousticDecision::Advanced => AcousticEmergencyDecision::PhraseAdvanced,
                    AcousticDecision::Triggered => {
                        self.request_reduction(window_end_sequence.saturating_add(1))
                    }
                    AcousticDecision::Suppressed => AcousticEmergencyDecision::Suppressed,
                    AcousticDecision::Waiting | AcousticDecision::Reset => {
                        AcousticEmergencyDecision::Waiting
                    }
                    AcousticDecision::Unavailable => self.provider_lost(),
                }
            }
            DetectorDecision::Refused(refusal) => self.detector_refused(refusal),
        }
    }

    pub fn physically_muted(&mut self) -> AcousticEmergencyDecision {
        self.availability = AcousticEmergencyAvailability::PhysicallyMuted;
        let _ = self.sequence.observe(AcousticObservation::MicrophoneLost);
        let _ = self.detector.microphone_lost();
        AcousticEmergencyDecision::Unavailable(self.availability)
    }

    pub fn provider_lost(&mut self) -> AcousticEmergencyDecision {
        self.availability = AcousticEmergencyAvailability::ProviderUnavailable;
        let _ = self.sequence.observe(AcousticObservation::MicrophoneLost);
        let _ = self.detector.microphone_lost();
        AcousticEmergencyDecision::Unavailable(self.availability)
    }

    pub fn input_overflow(&mut self) -> AcousticEmergencyDecision {
        self.availability = AcousticEmergencyAvailability::InputOverflow;
        let _ = self.sequence.observe(AcousticObservation::Overflow);
        let _ = self.detector.input_overflow();
        AcousticEmergencyDecision::Unavailable(self.availability)
    }

    fn detector_refused(&mut self, refusal: DetectorRefusal) -> AcousticEmergencyDecision {
        match refusal {
            DetectorRefusal::StaleGeneration => {
                self.availability = AcousticEmergencyAvailability::ProviderReplaced;
                let _ = self.sequence.observe(AcousticObservation::StaleGeneration);
                AcousticEmergencyDecision::Unavailable(self.availability)
            }
            DetectorRefusal::MicrophoneLost => self.provider_lost(),
            DetectorRefusal::InputOverflow => self.input_overflow(),
            DetectorRefusal::ReplayedFrame
            | DetectorRefusal::SequenceGap
            | DetectorRefusal::AmbiguousMatch
            | DetectorRefusal::DuplicateWord
            | DetectorRefusal::EmptyThreshold => {
                let _ = self
                    .sequence
                    .observe(AcousticObservation::InvalidConfidence);
                AcousticEmergencyDecision::Waiting
            }
        }
    }

    fn request_reduction(&mut self, freshness: u64) -> AcousticEmergencyDecision {
        let request = EmergencyRequest {
            request_id: format!("acoustic/{freshness}"),
            body_id: self.body_id.clone(),
            host_id: self.host_id.clone(),
            boot_id: self.boot_id.clone(),
            trigger: EmergencyTriggerClass::LocalAcousticEmergency,
            policy_id: EMERGENCY_CONTROL_POLICY.into(),
            freshness,
        };
        match self.control.inspect(&request) {
            Ok(outcome) => {
                self.triggered = true;
                AcousticEmergencyDecision::AuthorityReductionRequested(outcome)
            }
            Err(refusal) => AcousticEmergencyDecision::Refused(refusal),
        }
    }
}

#[cfg(test)]
mod tests;
