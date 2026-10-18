//! Phone-scoped energy preparation retains the immutable correspondence owner.
//! Audio executes quantity/trajectory laws; supplied observations do not rewrite
//! linguistic intent or target controls, and dB uncertainty is never inferred.
use crate::{correspondence::PreparedSpeechCorrespondence, semantic::*};
use conduit_audio::{
    convert_referenced_amplitude_to_power, AcousticQuantityRefusal, AudioDecibelRefusal,
    AudioRatioDecibelReceipt, AudioReferencedAmplitudePowerReceipt,
    AudioReferencedAmplitudePowerRequest, AudioTrajectoryAnchor, AudioTrajectoryRefusal,
    PreparedAudioQuantityTrajectory, PreparedExactPowerOfTenDecibels,
};
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};

#[derive(Debug)]
pub enum IntentEnergyRefusal {
    Event,
    MissingObservation,
    Occurrence,
    InventoryBasis,
    Source,
    Anchor,
    Native(NativeBindingRefusal),
    Trajectory(AudioTrajectoryRefusal),
    Decibel(AudioDecibelRefusal),
    Power(AcousticQuantityRefusal),
}
impl From<NativeBindingRefusal> for IntentEnergyRefusal {
    fn from(value: NativeBindingRefusal) -> Self {
        Self::Native(value)
    }
}

/// Preparation receipt. Decoding `SpeechEnergyBinding` alone does not validate
/// custody against a committed correspondence or execute trajectory admission.
pub struct PreparedIntentEnergy<'a, 'owner, 'inventory> {
    original: &'a PreparedSpeechCorrespondence<'owner, 'inventory>,
    event_index: usize,
    binding: SpeechEnergyBinding,
    trajectory: PreparedAudioQuantityTrajectory,
}
impl<'a, 'owner, 'inventory> PreparedIntentEnergy<'a, 'owner, 'inventory> {
    pub fn original(&self) -> &'a PreparedSpeechCorrespondence<'owner, 'inventory> {
        self.original
    }
    pub fn event_index(&self) -> usize {
        self.event_index
    }
    pub fn binding(&self) -> &SpeechEnergyBinding {
        &self.binding
    }
    pub fn target(&self) -> &SpeechEnergyTarget {
        self.binding.target()
    }
    pub fn observation(&self) -> Option<&SpeechEnergyObservation> {
        self.binding.observation().as_ref()
    }
    pub fn trajectory(&self) -> &PreparedAudioQuantityTrajectory {
        &self.trajectory
    }

    /// Explicit conversion of the supplied observation, retaining Audio's full
    /// original request and Source executions. Unsupported logarithms refuse.
    pub fn observed_decibels(&self) -> Result<AudioRatioDecibelReceipt, IntentEnergyRefusal> {
        let observation = self
            .observation()
            .ok_or(IntentEnergyRefusal::MissingObservation)?;
        PreparedExactPowerOfTenDecibels::new()
            .map_err(IntentEnergyRefusal::Decibel)?
            .ratio_to_decibels(&observation.value().clone().encode()?)
            .map_err(IntentEnergyRefusal::Decibel)
    }

    /// Conversion requires the separately supplied generic positive
    /// proportionality premise and paired amplitude/power references.
    pub fn observed_power(
        &self,
        request: &AudioReferencedAmplitudePowerRequest,
    ) -> Result<AudioReferencedAmplitudePowerReceipt, IntentEnergyRefusal> {
        let observation = self
            .observation()
            .ok_or(IntentEnergyRefusal::MissingObservation)?;
        SpeechEnergyPowerRequestMatch::new(observation.clone(), request.clone())?;
        convert_referenced_amplitude_to_power(&request.clone().encode()?)
            .map_err(IntentEnergyRefusal::Power)
    }
}

/// Supported subset: one original phone event, complete original text reference,
/// one explicit anchor, and one finite amplitude or power control trajectory.
/// The source reference establishes full utterance carrier custody. Actual word
/// source references remain separately inspectable through `original().intent()`.
/// A target requires no supplied observation; absence remains explicit.
/// Exact observation intervals use Audio offsets within the U32 cross-product
/// comparison profile. No signal measurement or physical calibration is inferred.
pub fn prepare_intent_energy<'a, 'owner, 'inventory>(
    original: &'a PreparedSpeechCorrespondence<'owner, 'inventory>,
    event_index: usize,
    anchor: &AudioTrajectoryAnchor,
    binding: &SpeechEnergyBinding,
) -> Result<PreparedIntentEnergy<'a, 'owner, 'inventory>, IntentEnergyRefusal> {
    // Re-admit serialized contracts even when callers retain Rust values.
    let binding = SpeechEnergyBinding::decode(&binding.clone().encode()?)?;
    let Some(SpeechUtteranceIntentEvent::Segment(segment)) =
        original.intent().events().as_slice().get(event_index)
    else {
        return Err(IntentEnergyRefusal::Event);
    };
    let scope = binding.target().scope();
    if scope.occurrence() != segment.occurrence() {
        return Err(IntentEnergyRefusal::Occurrence);
    }
    if scope.inventory_basis() != original.inventory().basis() {
        return Err(IntentEnergyRefusal::InventoryBasis);
    }
    if scope.source() != original.alignment().source() {
        return Err(IntentEnergyRefusal::Source);
    }
    if scope.anchor() != anchor {
        return Err(IntentEnergyRefusal::Anchor);
    }
    for segment in binding
        .target()
        .control()
        .trajectory()
        .segments()
        .as_slice()
    {
        for endpoint in [segment.left(), segment.right()] {
            SpeechEnergyEndpointMatch::new(endpoint.clone(), binding.target().clone())?;
        }
    }
    if let Some(observation) = binding.observation() {
        let check = |error| SpeechEnergyErrorMatch::new(error, observation.clone()).map(|_| ());
        match observation.absolute_linear_error() {
            SpeechEnergyErrorSpecification::Known(error) => check(error.clone())?,
            SpeechEnergyErrorSpecification::Variable(errors) => {
                for error in errors.as_slice() {
                    check(error.clone())?;
                }
            }
            SpeechEnergyErrorSpecification::Gradient(gradient) => check(gradient.value().clone())?,
            SpeechEnergyErrorSpecification::Unknown
            | SpeechEnergyErrorSpecification::Unspecified
            | SpeechEnergyErrorSpecification::NotApplicable => {}
        }
    }
    let trajectory = PreparedAudioQuantityTrajectory::new(
        &binding.target().control().trajectory().clone().encode()?,
    )
    .map_err(IntentEnergyRefusal::Trajectory)?;
    Ok(PreparedIntentEnergy {
        original,
        event_index,
        binding,
        trajectory,
    })
}
