//! Target F0 and separately supplied observation evidence through generic Audio
//! Source laws. No measurement authentication, commitment, model or playback
//! authority is established here.
use crate::{semantic::*, text_admission};
use alloc::vec::Vec;
use conduit_audio::*;
use conduit_language::LanguageText;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};

#[derive(Debug)]
pub enum IntentFundamentalRefusal {
    InventoryBasis {
        event: usize,
    },
    CarrierSource {
        event: usize,
    },
    MissingEvent {
        event: usize,
    },
    Boundary {
        event: usize,
    },
    ForeignOccurrence {
        event: usize,
    },
    MissingSource {
        event: usize,
    },
    NonTextSource {
        event: usize,
        source: usize,
    },
    Source {
        event: usize,
        source: usize,
        reason: text_admission::TextReferenceRefusal,
    },
    Occurrence {
        event: usize,
        reason: NativeBindingRefusal,
    },
    Unresolved {
        event: usize,
        specification: SpeechCycleSpecification,
    },
    CycleMismatch {
        event: usize,
    },
    MissingRate {
        event: usize,
    },
    ForeignAnchor {
        event: usize,
    },
    Native {
        event: usize,
        reason: NativeBindingRefusal,
    },
    Reciprocal {
        event: usize,
        reason: AcousticQuantityRefusal,
    },
    Projection {
        event: usize,
        reason: AudioRateProjectionRefusal,
    },
}

pub struct PreparedIntentFundamental<'a> {
    intent: &'a SpeechUtteranceIntent,
    segment: &'a SpeechUtteranceIntentEventSegment,
    text: &'a LanguageText,
    event: usize,
    control: &'a AudioFrequencyControl,
    occurrence: SpeechOccurrenceMembership,
    sources: Vec<text_admission::ResolvedText<'a>>,
    reciprocal: AcousticConversion<AudioFrequencyHz, AudioCycleDuration>,
    reverse: AcousticConversion<AudioCycleDuration, AudioFrequencyHz>,
}
impl<'a> PreparedIntentFundamental<'a> {
    pub fn intent(&self) -> &'a SpeechUtteranceIntent {
        self.intent
    }
    pub fn segment(&self) -> &'a SpeechUtteranceIntentEventSegment {
        self.segment
    }
    pub fn text(&self) -> &'a LanguageText {
        self.text
    }
    pub fn event(&self) -> usize {
        self.event
    }
    pub fn control(&self) -> &'a AudioFrequencyControl {
        self.control
    }
    pub fn occurrence(&self) -> &SpeechOccurrenceMembership {
        &self.occurrence
    }
    pub fn sources(&self) -> &[text_admission::ResolvedText<'a>] {
        &self.sources
    }
    pub fn reciprocal(&self) -> &AcousticConversion<AudioFrequencyHz, AudioCycleDuration> {
        &self.reciprocal
    }
    pub fn reverse(&self) -> &AcousticConversion<AudioCycleDuration, AudioFrequencyHz> {
        &self.reverse
    }
    /// Explicit abstract rate/anchor projection. Quantization is retained by Audio.
    pub fn at_rate(
        &self,
        basis: Option<&AudioSampleRateBasis>,
    ) -> Result<AudioSampleProjectionReceipt, IntentFundamentalRefusal> {
        let event = self.event;
        let basis = basis.ok_or(IntentFundamentalRefusal::MissingRate { event })?;
        if basis.anchor() != self.control.anchor() {
            return Err(IntentFundamentalRefusal::ForeignAnchor { event });
        }
        let request = AudioSampleProjectionRequest::new(
            basis.clone(),
            AudioSampleProjectionQuantity::cycle(
                *self.reciprocal.result().denominator(),
                *self.reciprocal.result().numerator_seconds(),
            )
            .map_err(|reason| IntentFundamentalRefusal::Native { event, reason })?,
        )
        .map_err(|reason| IntentFundamentalRefusal::Native { event, reason })?;
        let canonical = request
            .encode()
            .map_err(|reason| IntentFundamentalRefusal::Native { event, reason })?;
        PreparedAudioSampleProjection::new()
            .and_then(|projector| projector.project(&canonical))
            .map_err(|reason| IntentFundamentalRefusal::Projection { event, reason })
    }
}

/// Numeric preparation only: inventory membership and Variety are not admitted
/// here. Use `prepare_corresponding_intent_fundamental` for linguistic custody.
/// Exact representation agreement is required: equivalent but differently
/// represented fractions need a separately admitted map, never silent reduction.
pub fn prepare_intent_fundamental<'a>(
    intent: &'a SpeechUtteranceIntent,
    event: usize,
    occurrence: &LanguageSpeechTokenRef,
    text: &'a LanguageText,
    control: &'a AudioFrequencyControl,
) -> Result<PreparedIntentFundamental<'a>, IntentFundamentalRefusal> {
    let segment = match intent.events().as_slice().get(event) {
        Some(SpeechUtteranceIntentEvent::Segment(value)) => value,
        Some(_) => return Err(IntentFundamentalRefusal::Boundary { event }),
        None => return Err(IntentFundamentalRefusal::MissingEvent { event }),
    };
    if segment.occurrence() != occurrence {
        return Err(IntentFundamentalRefusal::ForeignOccurrence { event });
    }
    let occurrence = SpeechOccurrenceMembership::new(
        intent.inventory_id().clone(),
        intent.language().clone(),
        occurrence.clone(),
        intent.revision_id().clone(),
        intent.utterance_id().clone(),
    )
    .map_err(|reason| IntentFundamentalRefusal::Occurrence { event, reason })?;
    if segment.sources().as_slice().is_empty() {
        return Err(IntentFundamentalRefusal::MissingSource { event });
    }
    let mut sources = Vec::with_capacity(segment.sources().len());
    for (source, reference) in segment.sources().as_slice().iter().enumerate() {
        let LanguageSegmentRef::Text(_) = reference else {
            return Err(IntentFundamentalRefusal::NonTextSource { event, source });
        };
        sources.push(
            text_admission::resolve_text(reference, text).map_err(|reason| {
                IntentFundamentalRefusal::Source {
                    event,
                    source,
                    reason,
                }
            })?,
        );
    }
    let SpeechCycleSpecification::Known(cycle) = segment.prosody().fundamental_cycle() else {
        return Err(IntentFundamentalRefusal::Unresolved {
            event,
            specification: segment.prosody().fundamental_cycle().clone(),
        });
    };
    let canonical = (*control.value())
        .encode()
        .map_err(|reason| IntentFundamentalRefusal::Native { event, reason })?;
    let converter = PreparedAcousticReciprocal::new()
        .map_err(|reason| IntentFundamentalRefusal::Reciprocal { event, reason })?;
    let reciprocal = converter
        .frequency_to_cycle(&canonical)
        .map_err(|reason| IntentFundamentalRefusal::Reciprocal { event, reason })?;
    if cycle.numerator_seconds() != reciprocal.result().numerator_seconds()
        || cycle.denominator() != reciprocal.result().denominator()
    {
        return Err(IntentFundamentalRefusal::CycleMismatch { event });
    }
    let reverse = converter
        .cycle_to_frequency(reciprocal.result_canonical())
        .map_err(|reason| IntentFundamentalRefusal::Reciprocal { event, reason })?;
    Ok(PreparedIntentFundamental {
        intent,
        segment,
        text,
        event,
        control,
        occurrence,
        sources,
        reciprocal,
        reverse,
    })
}

/// Supplied observation evidence, distinct from the authored target. This
/// authenticates no sensor and performs no measurement or target substitution.
pub struct ObservedIntentFundamental<'a> {
    target: &'a PreparedIntentFundamental<'a>,
    observed: &'a AudioFrequencyControl,
    at: &'a AudioTrajectoryQuery,
    span: &'a SpeechExactDuration,
    provenance: &'a SpeechEvidenceProvenance,
    confidence: Option<&'a SpeechConfidence>,
    reciprocal: AcousticConversion<AudioFrequencyHz, AudioCycleDuration>,
}
impl<'a> ObservedIntentFundamental<'a> {
    pub fn target(&self) -> &'a PreparedIntentFundamental<'a> {
        self.target
    }
    pub fn observed(&self) -> &'a AudioFrequencyControl {
        self.observed
    }
    pub fn at(&self) -> &'a AudioTrajectoryQuery {
        self.at
    }
    pub fn span(&self) -> &'a SpeechExactDuration {
        self.span
    }
    pub fn provenance(&self) -> &'a SpeechEvidenceProvenance {
        self.provenance
    }
    pub fn confidence(&self) -> Option<&'a SpeechConfidence> {
        self.confidence
    }
    pub fn reciprocal(&self) -> &AcousticConversion<AudioFrequencyHz, AudioCycleDuration> {
        &self.reciprocal
    }
}
pub fn bind_observed_intent_fundamental<'a>(
    target: &'a PreparedIntentFundamental<'a>,
    observed: &'a AudioFrequencyControl,
    at: &'a AudioTrajectoryQuery,
    span: &'a SpeechExactDuration,
    provenance: &'a SpeechEvidenceProvenance,
    confidence: Option<&'a SpeechConfidence>,
) -> Result<ObservedIntentFundamental<'a>, IntentFundamentalRefusal> {
    let event = target.event();
    if observed.anchor() != target.control().anchor() || at.anchor() != observed.anchor() {
        return Err(IntentFundamentalRefusal::ForeignAnchor { event });
    }
    let canonical = (*observed.value())
        .encode()
        .map_err(|reason| IntentFundamentalRefusal::Native { event, reason })?;
    let reciprocal = PreparedAcousticReciprocal::new()
        .and_then(|converter| converter.frequency_to_cycle(&canonical))
        .map_err(|reason| IntentFundamentalRefusal::Reciprocal { event, reason })?;
    Ok(ObservedIntentFundamental {
        target,
        observed,
        at,
        span,
        provenance,
        confidence,
        reciprocal,
    })
}

/// Full upstream inventory/Variety/revision custody retained alongside numeric
/// target preparation. This does not add commitment or source authentication.
pub struct CorrespondingIntentFundamental<'a, 'inventory> {
    carrier: &'a crate::correspondence::PreparedSpeechCorrespondence<'a, 'inventory>,
    fundamental: PreparedIntentFundamental<'a>,
}
impl<'a, 'inventory> CorrespondingIntentFundamental<'a, 'inventory> {
    pub fn carrier(
        &self,
    ) -> &'a crate::correspondence::PreparedSpeechCorrespondence<'a, 'inventory> {
        self.carrier
    }
    pub fn fundamental(&self) -> &PreparedIntentFundamental<'a> {
        &self.fundamental
    }
    /// A supplied observation retains the same admitted inventory/Variety owner.
    pub fn observe<'r>(
        &'r self,
        observed: &'r AudioFrequencyControl,
        at: &'r AudioTrajectoryQuery,
        span: &'r SpeechExactDuration,
        provenance: &'r SpeechEvidenceProvenance,
        confidence: Option<&'r SpeechConfidence>,
    ) -> Result<CorrespondingObservedFundamental<'r, 'inventory>, IntentFundamentalRefusal> {
        let observation = bind_observed_intent_fundamental(
            &self.fundamental,
            observed,
            at,
            span,
            provenance,
            confidence,
        )?;
        Ok(CorrespondingObservedFundamental {
            carrier: self.carrier,
            observation,
        })
    }
}
pub fn prepare_corresponding_intent_fundamental<'a, 'inventory>(
    carrier: &'a crate::correspondence::PreparedSpeechCorrespondence<'a, 'inventory>,
    event: usize,
    control: &'a AudioFrequencyControl,
) -> Result<CorrespondingIntentFundamental<'a, 'inventory>, IntentFundamentalRefusal> {
    let occurrence = match carrier.intent().events().as_slice().get(event) {
        Some(SpeechUtteranceIntentEvent::Segment(segment)) => segment.occurrence(),
        Some(_) => return Err(IntentFundamentalRefusal::Boundary { event }),
        None => return Err(IntentFundamentalRefusal::MissingEvent { event }),
    };
    let fundamental =
        prepare_intent_fundamental(carrier.intent(), event, occurrence, carrier.text(), control)?;
    Ok(CorrespondingIntentFundamental {
        carrier,
        fundamental,
    })
}

/// Separately supplied observation, retaining full original linguistic custody.
pub struct CorrespondingObservedFundamental<'a, 'inventory> {
    carrier: &'a crate::correspondence::PreparedSpeechCorrespondence<'a, 'inventory>,
    observation: ObservedIntentFundamental<'a>,
}
impl<'a, 'inventory> CorrespondingObservedFundamental<'a, 'inventory> {
    pub fn carrier(
        &self,
    ) -> &'a crate::correspondence::PreparedSpeechCorrespondence<'a, 'inventory> {
        self.carrier
    }
    pub fn observation(&self) -> &ObservedIntentFundamental<'a> {
        &self.observation
    }
}

/// Authored target and optional supplied observation, tied to the complete opaque
/// original correspondence. Source identifies carrier custody; event word source
/// references remain inspectable on `original().intent()`.
pub struct PreparedPhoneFundamental<'a, 'inventory> {
    original: &'a crate::correspondence::PreparedSpeechCorrespondence<'a, 'inventory>,
    binding: &'a SpeechFundamentalBinding,
    target: CorrespondingIntentFundamental<'a, 'inventory>,
    observed_cycle: Option<AcousticConversion<AudioFrequencyHz, AudioCycleDuration>>,
}
impl<'a, 'inventory> PreparedPhoneFundamental<'a, 'inventory> {
    pub fn original(
        &self,
    ) -> &'a crate::correspondence::PreparedSpeechCorrespondence<'a, 'inventory> {
        self.original
    }
    pub fn binding(&self) -> &'a SpeechFundamentalBinding {
        self.binding
    }
    pub fn target(&self) -> &CorrespondingIntentFundamental<'a, 'inventory> {
        &self.target
    }
    pub fn observation(&self) -> Option<&SpeechFundamentalObservation> {
        self.binding.observation().as_ref()
    }
    pub fn observed_cycle(
        &self,
    ) -> Option<&AcousticConversion<AudioFrequencyHz, AudioCycleDuration>> {
        self.observed_cycle.as_ref()
    }
}

/// Supported subset: a single original phone event with a known exact intended
/// cycle. Source laws retain dimensions and anchor relations; generic Audio runs
/// each reciprocal conversion. No frequency/error arithmetic is reimplemented.
pub fn prepare_phone_fundamental<'a, 'inventory>(
    original: &'a crate::correspondence::PreparedSpeechCorrespondence<'a, 'inventory>,
    event: usize,
    binding: &'a SpeechFundamentalBinding,
) -> Result<PreparedPhoneFundamental<'a, 'inventory>, IntentFundamentalRefusal> {
    SpeechFundamentalBinding::decode(
        &binding
            .clone()
            .encode()
            .map_err(|reason| IntentFundamentalRefusal::Native { event, reason })?,
    )
    .map_err(|reason| IntentFundamentalRefusal::Native { event, reason })?;
    let scope = binding.target().scope();
    if scope.inventory_basis() != original.inventory().basis() {
        return Err(IntentFundamentalRefusal::InventoryBasis { event });
    }
    if scope.source() != original.alignment().source() {
        return Err(IntentFundamentalRefusal::CarrierSource { event });
    }
    let target =
        prepare_corresponding_intent_fundamental(original, event, binding.target().control())?;
    if scope.occurrence() != target.fundamental().segment().occurrence() {
        return Err(IntentFundamentalRefusal::ForeignOccurrence { event });
    }
    let observed_cycle = binding
        .observation()
        .as_ref()
        .map(|observed| {
            let bytes = (*observed.control().value())
                .encode()
                .map_err(|reason| IntentFundamentalRefusal::Native { event, reason })?;
            PreparedAcousticReciprocal::new()
                .and_then(|converter| converter.frequency_to_cycle(&bytes))
                .map_err(|reason| IntentFundamentalRefusal::Reciprocal { event, reason })
        })
        .transpose()?;
    Ok(PreparedPhoneFundamental {
        original,
        binding,
        target,
        observed_cycle,
    })
}
