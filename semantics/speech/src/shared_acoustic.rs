//! Allocating composition over the original IPA/shared owner and acoustic ports.
//! Scope labels do not prove segmentation; this checks exact retained references.
use crate::{
    common_acoustic_curves::*, common_acoustic_evidence::*, common_acoustic_quantities::*,
    common_audio_targets::*, ipa_shared::*, reference_admission::*, semantic::*, text_admission::*,
};
use alloc::{boxed::Box, vec::Vec};
use conduit_audio::{AudioQuantityTrajectory, AudioTrajectoryAnchor};
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};
pub struct SpeechAcousticComponents<'a> {
    pub timing_pitch: &'a [SpeechTimingPitchTargets],
    pub linguistic_rate: &'a [SpeechLinguisticRateTargets],
    pub intensity: &'a [SpeechIntensityTargets],
    pub formants: &'a [SpeechFormantTargets],
    pub voice_quality: &'a [SpeechVoiceQualityTargets],
    pub observations: &'a [SpeechCommonAcousticEvidence],
}
#[derive(Debug)]
pub enum SharedAcousticRefusal {
    Native(NativeBindingRefusal),
    Common(SpeechCommonAcousticRefusal),
    Audio(SpeechAudioTargetRefusal),
    MissingSource,
    ForeignAnchor,
    Representation,
}
impl From<NativeBindingRefusal> for SharedAcousticRefusal {
    fn from(v: NativeBindingRefusal) -> Self {
        Self::Native(v)
    }
}
impl From<SpeechCommonAcousticRefusal> for SharedAcousticRefusal {
    fn from(v: SpeechCommonAcousticRefusal) -> Self {
        Self::Common(v)
    }
}
impl From<SpeechAudioTargetRefusal> for SharedAcousticRefusal {
    fn from(v: SpeechAudioTargetRefusal) -> Self {
        Self::Audio(v)
    }
}
pub enum AcousticResolvedSource<'a> {
    Phone(ResolvedPhone<'a>),
    Phoneme(ResolvedPhoneme<'a>),
    Text(ResolvedText<'a>),
    OriginalEvent {
        event: &'a SpeechUtteranceIntentEvent,
        witness: Box<SpeechAcousticEventSourceMatch>,
    },
}
pub struct AcousticScopeReceipt<'a> {
    scope: &'a SpeechAcousticScope,
    memberships: Vec<SpeechAcousticScopeSourceMatch>,
    resolved: Vec<AcousticResolvedSource<'a>>,
}
impl<'a> AcousticScopeReceipt<'a> {
    pub fn scope(&self) -> &'a SpeechAcousticScope {
        self.scope
    }
    pub fn memberships(&self) -> &[SpeechAcousticScopeSourceMatch] {
        &self.memberships
    }
    pub fn resolved(&self) -> &[AcousticResolvedSource<'a>] {
        &self.resolved
    }
}
pub enum AcousticFieldPreparation {
    Audio(Box<PreparedSpeechAudioTarget>),
    StaticDuration(
        SpeechCommonQuantityReceipt<SpeechExactDuration, conduit_audio::AudioTimeFraction>,
    ),
    Duration(PreparedSpeechTypedStepCurve<SpeechDurationCurve>),
    Rate(PreparedSpeechTypedStepCurve<SpeechSyllabicRateCurve>),
    Decibels(PreparedSpeechTypedStepCurve<SpeechDecibelLevelCurve>),
    Probability(PreparedSpeechTypedStepCurve<SpeechProbabilityCurve>),
    Tilt(PreparedSpeechTypedStepCurve<SpeechSpectralTiltCurve>),
}
struct AudioCandidateField {
    component: &'static str,
    ordinal: usize,
    field: &'static str,
    role: SpeechTargetAudioRole,
    formant_index: Option<u32>,
}
pub struct AcousticCandidatePreparation {
    pub component: &'static str,
    pub ordinal: usize,
    pub field: &'static str,
    pub candidate: usize,
    pub formant_index: Option<u32>,
    pub preparation: AcousticFieldPreparation,
}
pub struct PreparedSharedAcousticIntent<'a, 'joined, 'material> {
    joined: &'a PreparedIpaSpeechUtteranceIntent<'joined, 'material>,
    components: SpeechAcousticComponents<'a>,
    anchor: &'a AudioTrajectoryAnchor,
    counts: SpeechSharedAcousticComponentCounts,
    scopes: Vec<AcousticScopeReceipt<'a>>,
    executions: Vec<SpeechCommonAcousticExecution>,
    candidates: Vec<AcousticCandidatePreparation>,
    observations: Vec<SpeechCommonAcousticEvidenceReceipt>,
}
// Known payload wrappers expose fields but intentionally have no standalone
// Native binding. Copy every original field into the original checked owner.
macro_rules! known_curve {
    ($name:ident,$wrapper:ty,$owner:ident) => {
        fn $name(value: &$wrapper) -> Result<$owner, SharedAcousticRefusal> {
            Ok($owner::new(
                value.anchor().clone(),
                *value.endpoints(),
                *value.outside(),
                value.provenance().clone(),
                value.segments().clone(),
            )?)
        }
    };
}
known_curve!(
    known_audio,
    SpeechAudioTrajectorySpecificationKnown,
    AudioQuantityTrajectory
);
known_curve!(
    known_duration_curve,
    SpeechDurationCurveSpecificationKnown,
    SpeechDurationCurve
);
known_curve!(
    known_rate_curve,
    SpeechRateCurveSpecificationKnown,
    SpeechSyllabicRateCurve
);
known_curve!(
    known_decibel_curve,
    SpeechDecibelCurveSpecificationKnown,
    SpeechDecibelLevelCurve
);
known_curve!(
    known_probability_curve,
    SpeechProbabilityCurveSpecificationKnown,
    SpeechProbabilityCurve
);
known_curve!(
    known_tilt_curve,
    SpeechSpectralTiltCurveSpecificationKnown,
    SpeechSpectralTiltCurve
);
fn known_duration(
    value: &SpeechDurationSpecificationKnown,
) -> Result<SpeechExactDuration, SharedAcousticRefusal> {
    Ok(SpeechExactDuration::new(
        *value.denominator(),
        *value.numerator_seconds(),
    )?)
}
macro_rules! candidates {
    ($value:expr, $spec:ident, $known:ident, $body:expr) => {{
        let mut visit = $body;
        match $value {
            $spec::Known(v) => visit(0, &$known(v)?)?,
            $spec::Variable(v) => {
                for (n, v) in v.as_slice().iter().enumerate() {
                    visit(n, v)?;
                }
            }
            $spec::Gradient(v) => visit(0, v.value())?,
            $spec::Unknown | $spec::Unspecified | $spec::NotApplicable => {}
        }
    }};
}
impl<'a, 'joined, 'material> PreparedSharedAcousticIntent<'a, 'joined, 'material>
where
    'material: 'a,
    'joined: 'a,
{
    pub fn joined(&self) -> &'a PreparedIpaSpeechUtteranceIntent<'joined, 'material> {
        self.joined
    }
    pub fn components(&self) -> &SpeechAcousticComponents<'a> {
        &self.components
    }
    pub fn anchor(&self) -> &'a AudioTrajectoryAnchor {
        self.anchor
    }
    pub fn counts(&self) -> &SpeechSharedAcousticComponentCounts {
        &self.counts
    }
    pub fn scopes(&self) -> &[AcousticScopeReceipt<'a>] {
        &self.scopes
    }
    pub fn executions(&self) -> &[SpeechCommonAcousticExecution] {
        &self.executions
    }
    pub fn candidates(&self) -> &[AcousticCandidatePreparation] {
        &self.candidates
    }
    pub fn observations(&self) -> &[SpeechCommonAcousticEvidenceReceipt] {
        &self.observations
    }
    fn anchor_match(
        &mut self,
        anchor: &AudioTrajectoryAnchor,
    ) -> Result<(), SharedAcousticRefusal> {
        if !boolean(
            crate::common_acoustic_programs::ANCHOR,
            SpeechAcousticAnchorComparison::new(self.anchor.clone(), anchor.clone())?,
            &mut self.executions,
        )? {
            return Err(SharedAcousticRefusal::ForeignAnchor);
        }
        Ok(())
    }
    fn scope(&mut self, scope: &'a SpeechAcousticScope) -> Result<(), SharedAcousticRefusal> {
        let shared = self.joined.shared();
        let material = shared.components();
        let mut receipt = AcousticScopeReceipt {
            scope,
            memberships: Vec::new(),
            resolved: Vec::new(),
        };
        for (index, reference) in scope.sources().as_slice().iter().enumerate() {
            receipt
                .memberships
                .push(SpeechAcousticScopeSourceMatch::new(
                    index as u64,
                    reference.clone(),
                    scope.clone(),
                )?);
            let resolved = match reference {
                LanguageSegmentRef::Phone(_) => AcousticResolvedSource::Phone(
                    resolve_phone(reference, material.phones)
                        .map_err(|_| SharedAcousticRefusal::MissingSource)?,
                ),
                LanguageSegmentRef::Phoneme(_) => AcousticResolvedSource::Phoneme(
                    resolve_phoneme(reference, material.phonemes)
                        .map_err(|_| SharedAcousticRefusal::MissingSource)?,
                ),
                LanguageSegmentRef::Text(_) => {
                    let found = material
                        .intended_text
                        .into_iter()
                        .chain(material.morpheme_texts.iter().copied().flatten())
                        .find_map(|text| resolve_text(reference, text).ok())
                        .ok_or(SharedAcousticRefusal::MissingSource)?;
                    AcousticResolvedSource::Text(found)
                }
                _ => {
                    let mut found = None;
                    for event in shared.original().events().as_slice() {
                        let sources = match event {
                            SpeechUtteranceIntentEvent::Segment(e) => e.sources(),
                            SpeechUtteranceIntentEvent::Boundary(e) => e.sources(),
                        };
                        for (n, source) in sources.as_slice().iter().enumerate() {
                            if source == reference {
                                found = Some(AcousticResolvedSource::OriginalEvent {
                                    event,
                                    witness: Box::new(SpeechAcousticEventSourceMatch::new(
                                        event.clone(),
                                        n as u64,
                                        reference.clone(),
                                    )?),
                                });
                                break;
                            }
                        }
                        if found.is_some() {
                            break;
                        }
                    }
                    found.ok_or(SharedAcousticRefusal::MissingSource)?
                }
            };
            receipt.resolved.push(resolved);
        }
        self.scopes.push(receipt);
        Ok(())
    }
    fn audio(
        &mut self,
        descriptor: AudioCandidateField,
        scope: &'a SpeechAcousticScope,
        provenance: &SpeechEvidenceProvenance,
        spec: &'a SpeechAudioTrajectorySpecification,
    ) -> Result<(), SharedAcousticRefusal> {
        let AudioCandidateField {
            component,
            ordinal,
            field,
            role,
            formant_index,
        } = descriptor;
        candidates!(
            spec,
            SpeechAudioTrajectorySpecification,
            known_audio,
            |candidate, value: &AudioQuantityTrajectory| -> Result<(), SharedAcousticRefusal> {
                self.anchor_match(value.anchor())?;
                let request = SpeechKnownAudioTarget::new(
                    provenance.clone(),
                    role,
                    scope.clone(),
                    value.clone(),
                )?;
                self.candidates.push(AcousticCandidatePreparation {
                    component,
                    ordinal,
                    field,
                    candidate,
                    formant_index,
                    preparation: AcousticFieldPreparation::Audio(Box::new(
                        PreparedSpeechAudioTarget::new(&request.encode()?)?,
                    )),
                });
                Ok(())
            }
        );
        Ok(())
    }
    pub fn prepare(
        joined: &'a PreparedIpaSpeechUtteranceIntent<'joined, 'material>,
        anchor: &'a AudioTrajectoryAnchor,
        components: SpeechAcousticComponents<'a>,
    ) -> Result<Self, SharedAcousticRefusal> {
        let count = |v: usize| u64::try_from(v).map_err(|_| SharedAcousticRefusal::Representation);
        let counts = SpeechSharedAcousticComponentCounts::new(
            count(components.formants.len())?,
            count(components.intensity.len())?,
            count(components.linguistic_rate.len())?,
            count(components.observations.len())?,
            count(components.timing_pitch.len())?,
            count(components.voice_quality.len())?,
        )?;
        let timing_pitch = components.timing_pitch;
        let linguistic_rate = components.linguistic_rate;
        let intensity = components.intensity;
        let formants = components.formants;
        let voice_quality = components.voice_quality;
        let observations = components.observations;
        let mut result = Self {
            joined,
            components,
            anchor,
            counts,
            scopes: Vec::new(),
            executions: Vec::new(),
            candidates: Vec::new(),
            observations: Vec::new(),
        };
        macro_rules! curve {
            ($component:expr,$ordinal:expr,$field:expr,$spec:expr,$spec_type:ident,$curve_type:ty,$prepare:ident,$known:ident,$variant:ident) => {
                candidates!($spec, $spec_type, $known, |candidate,
                                                        value: &$curve_type|
                 -> Result<
                    (),
                    SharedAcousticRefusal,
                > {
                    result.anchor_match(value.anchor())?;
                    result.candidates.push(AcousticCandidatePreparation {
                        component: $component,
                        ordinal: $ordinal,
                        field: $field,
                        candidate,
                        formant_index: None,
                        preparation: AcousticFieldPreparation::$variant($prepare(
                            &value.clone().encode()?,
                        )?),
                    });
                    Ok(())
                });
            };
        }
        for (n, target) in timing_pitch.iter().enumerate() {
            result.scope(target.scope())?;
            result.anchor_match(target.anchor())?;
            result.audio(
                AudioCandidateField {
                    component: "timing_pitch",
                    ordinal: n,
                    field: "pitch",
                    role: SpeechTargetAudioRole::Pitch,
                    formant_index: None,
                },
                target.scope(),
                target.provenance(),
                target.pitch(),
            )?;
            candidates!(
                target.duration(),
                SpeechDurationSpecification,
                known_duration,
                |candidate, value: &SpeechExactDuration| -> Result<(), SharedAcousticRefusal> {
                    result.candidates.push(AcousticCandidatePreparation {
                        component: "timing_pitch",
                        ordinal: n,
                        field: "duration",
                        candidate,
                        formant_index: None,
                        preparation: AcousticFieldPreparation::StaticDuration(
                            speech_duration_to_audio(&value.clone().encode()?)?,
                        ),
                    });
                    Ok(())
                }
            );
            curve!(
                "timing_pitch",
                n,
                "duration_curve",
                target.duration_curve(),
                SpeechDurationCurveSpecification,
                SpeechDurationCurve,
                prepare_speech_duration_step_curve,
                known_duration_curve,
                Duration
            );
        }
        for (n, target) in linguistic_rate.iter().enumerate() {
            result.scope(target.scope())?;
            result.anchor_match(target.anchor())?;
            let mut check_scopes = |segments: &'a [SpeechSyllabicRateCurveSegment]| -> Result<(), SharedAcousticRefusal> {
                for segment in segments {
                    match segment.value() {
                        SpeechSyllabicRateSpecification::Known(rate) => result.scope(rate.scope())?,
                        SpeechSyllabicRateSpecification::Variable(rates) => { for rate in rates.as_slice() { result.scope(rate.scope())?; } },
                        SpeechSyllabicRateSpecification::Gradient(rate) => result.scope(rate.value().scope())?,
                        SpeechSyllabicRateSpecification::Unknown | SpeechSyllabicRateSpecification::Unspecified | SpeechSyllabicRateSpecification::NotApplicable => {},
                    }
                }
                Ok(())
            };
            match target.speaking_rate() {
                SpeechRateCurveSpecification::Known(curve) => {
                    check_scopes(curve.segments().as_slice())?
                }
                SpeechRateCurveSpecification::Variable(curves) => {
                    for curve in curves.as_slice() {
                        check_scopes(curve.segments().as_slice())?;
                    }
                }
                SpeechRateCurveSpecification::Gradient(curve) => {
                    check_scopes(curve.value().segments().as_slice())?
                }
                SpeechRateCurveSpecification::Unknown
                | SpeechRateCurveSpecification::Unspecified
                | SpeechRateCurveSpecification::NotApplicable => {}
            }
            curve!(
                "linguistic_rate",
                n,
                "speaking_rate",
                target.speaking_rate(),
                SpeechRateCurveSpecification,
                SpeechSyllabicRateCurve,
                prepare_speech_rate_step_curve,
                known_rate_curve,
                Rate
            );
        }
        for (n, target) in intensity.iter().enumerate() {
            result.scope(target.scope())?;
            result.anchor_match(target.anchor())?;
            result.audio(
                AudioCandidateField {
                    component: "intensity",
                    ordinal: n,
                    field: "amplitude_power",
                    role: SpeechTargetAudioRole::Intensity,
                    formant_index: None,
                },
                target.scope(),
                target.provenance(),
                target.amplitude_power(),
            )?;
            curve!(
                "intensity",
                n,
                "decibels",
                target.decibels(),
                SpeechDecibelCurveSpecification,
                SpeechDecibelLevelCurve,
                prepare_speech_decibel_step_curve,
                known_decibel_curve,
                Decibels
            );
        }
        for (n, target) in formants.iter().enumerate() {
            result.scope(target.scope())?;
            result.anchor_match(target.anchor())?;
            for formant in target.formants().as_slice() {
                result.audio(
                    AudioCandidateField {
                        component: "formants",
                        ordinal: n,
                        field: "center",
                        role: SpeechTargetAudioRole::FormantCenter,
                        formant_index: Some(*formant.index()),
                    },
                    target.scope(),
                    formant.provenance(),
                    formant.center(),
                )?;
                result.audio(
                    AudioCandidateField {
                        component: "formants",
                        ordinal: n,
                        field: "bandwidth",
                        role: SpeechTargetAudioRole::FormantBandwidth,
                        formant_index: Some(*formant.index()),
                    },
                    target.scope(),
                    formant.provenance(),
                    formant.bandwidth(),
                )?;
            }
        }
        for (n, target) in voice_quality.iter().enumerate() {
            result.scope(target.scope())?;
            result.anchor_match(target.anchor())?;
            curve!(
                "voice_quality",
                n,
                "voicing_probability",
                target.voicing_probability(),
                SpeechProbabilityCurveSpecification,
                SpeechProbabilityCurve,
                prepare_speech_probability_step_curve,
                known_probability_curve,
                Probability
            );
            curve!(
                "voice_quality",
                n,
                "periodicity",
                target.periodicity(),
                SpeechProbabilityCurveSpecification,
                SpeechProbabilityCurve,
                prepare_speech_probability_step_curve,
                known_probability_curve,
                Probability
            );
            curve!(
                "voice_quality",
                n,
                "spectral_tilt",
                target.spectral_tilt(),
                SpeechSpectralTiltCurveSpecification,
                SpeechSpectralTiltCurve,
                prepare_speech_tilt_step_curve,
                known_tilt_curve,
                Tilt
            );
        }
        for evidence in observations {
            result.anchor_match(evidence.span().anchor())?;
            result.observations.push(prepare_speech_acoustic_evidence(
                &evidence.clone().encode()?,
            )?);
        }
        Ok(result)
    }
}
