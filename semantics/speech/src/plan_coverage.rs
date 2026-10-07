//! Complete Source participation and exact native phone coverage.
//! This admission grants no parser commitment or playback authority.
use crate::{
    pronunciation_intent::PreparedPronunciationIntent, semantic::*,
    text_token_role::PreparedTextTokenRole,
};
use alloc::vec::Vec;
use conduit_language::lexical::PreparedLexicalTape;
use conduit_plot::rust_binding::NativeBindingRefusal;

#[derive(Debug)]
pub enum SpeechPlanCoverageRefusal {
    Capacity,
    ForeignIntent,
    MissingRole,
    ForeignRole { token: usize },
    NativeOrder(NativeBindingRefusal),
    NativeSegment(NativeBindingRefusal),
    MissingWord,
    ForeignWord { word: usize },
    PhoneCount,
    Phone { event: usize },
    Occurrence { event: usize },
}

/// Preserve every checked field when the event variant and named segment use
/// distinct Native root schemas. The named Source constructor admits the full
/// candidate; this performs no canonical retyping or refinement erasure.
pub fn admit_planned_segment_material(
    segment: &SpeechUtteranceIntentEventSegment,
) -> Result<SpeechPlannedSegmentIntent, SpeechPlanCoverageRefusal> {
    SpeechPlannedSegmentIntent::new(
        segment.occurrence().clone(),
        segment.phone().clone(),
        segment.phoneme().clone(),
        segment.prosody().clone(),
        segment.provenance().clone(),
        segment.sources().clone(),
        segment.stress().clone(),
        segment.word_position().clone(),
    )
    .map_err(SpeechPlanCoverageRefusal::NativeSegment)
}

pub struct PreparedSpeechSpokenOrder<'a> {
    lexical: &'a PreparedLexicalTape,
    roles: &'a [PreparedTextTokenRole],
    native: SpeechCompleteSpokenOrder4,
    ordinals: Vec<usize>,
}
impl<'a> PreparedSpeechSpokenOrder<'a> {
    pub fn lexical(&self) -> &'a PreparedLexicalTape {
        self.lexical
    }
    pub fn roles(&self) -> &'a [PreparedTextTokenRole] {
        self.roles
    }
    pub fn native(&self) -> &SpeechCompleteSpokenOrder4 {
        &self.native
    }
    pub fn ordinals(&self) -> &[usize] {
        &self.ordinals
    }
}

/// Source already classified every token. Its complete order contract admits a
/// lexical order of all spoken tokens, including explicit nonspoken punctuation.
/// A caller-supplied subset cannot pass the Source count/bijection laws.
pub fn prepare_complete_spoken_order<'a>(
    lexical: &'a PreparedLexicalTape,
    roles: &'a [PreparedTextTokenRole],
    ordered: &[usize],
) -> Result<PreparedSpeechSpokenOrder<'a>, SpeechPlanCoverageRefusal> {
    use SpeechPlanCoverageRefusal::*;
    let tape = lexical.tape();
    let count = tape.tokens().len();
    if count == 0 || count > 4 || ordered.len() > 4 {
        return Err(Capacity);
    }
    if roles.len() != count {
        return Err(MissingRole);
    }
    let mut spoken = [false; 4];
    for (index, (token, role)) in tape.tokens().iter().zip(roles).enumerate() {
        let request = role.request();
        if request.source() != tape.source()
            || request.token() != token
            || request.analysis() != roles[0].request().analysis()
            || matches!(role.result().role(), SpeechTextTokenRole::Refused)
        {
            return Err(ForeignRole { token: index });
        }
        spoken[index] = matches!(role.result().role(), SpeechTextTokenRole::Spoken);
    }
    let mut slots = [0u64; 4];
    for (slot, ordinal) in slots.iter_mut().zip(ordered) {
        *slot = u64::try_from(*ordinal).map_err(|_| Capacity)?;
    }
    let native = SpeechCompleteSpokenOrder4::new(ordered.len() as u64, slots, spoken, count as u64)
        .map_err(NativeOrder)?;
    Ok(PreparedSpeechSpokenOrder {
        lexical,
        roles,
        native,
        ordinals: ordered.to_vec(),
    })
}

pub struct PreparedSpeechPlanCoverage<'a, 'word, 'basis> {
    order: &'a PreparedSpeechSpokenOrder<'a>,
    words: Vec<&'a PreparedPronunciationIntent<'word, 'basis>>,
    intent: &'a SpeechUtteranceIntent,
    layout: SpeechCompletePhoneLayout4,
    witnesses: &'a [SpeechPhoneCompositionWitness],
    phone_events: Vec<usize>,
}
impl<'a, 'word, 'basis> PreparedSpeechPlanCoverage<'a, 'word, 'basis> {
    pub fn order(&self) -> &PreparedSpeechSpokenOrder<'a> {
        self.order
    }
    pub fn words(&self) -> &[&'a PreparedPronunciationIntent<'word, 'basis>] {
        &self.words
    }
    pub fn intent(&self) -> &'a SpeechUtteranceIntent {
        self.intent
    }
    pub fn layout(&self) -> &SpeechCompletePhoneLayout4 {
        &self.layout
    }
    pub fn witnesses(&self) -> &'a [SpeechPhoneCompositionWitness] {
        self.witnesses
    }
    pub fn phone_events(&self) -> &[usize] {
        &self.phone_events
    }
}

/// Source admits offsets from the exact complete prepared word lengths.
pub fn prepare_complete_phone_layout<'a, 'word, 'basis>(
    order: &PreparedSpeechSpokenOrder<'a>,
    words: &[&'a PreparedPronunciationIntent<'word, 'basis>],
) -> Result<SpeechCompletePhoneLayout4, SpeechPlanCoverageRefusal> {
    validated_words(order, words).map(|(layout, _)| layout)
}

type ValidatedWordSegments = (
    SpeechCompletePhoneLayout4,
    Vec<(usize, SpeechPlannedSegmentIntent)>,
);

fn validated_words<'a, 'word, 'basis>(
    order: &PreparedSpeechSpokenOrder<'a>,
    words: &[&'a PreparedPronunciationIntent<'word, 'basis>],
) -> Result<ValidatedWordSegments, SpeechPlanCoverageRefusal> {
    use SpeechPlanCoverageRefusal::*;
    if words.len() != order.ordinals.len() {
        return Err(MissingWord);
    }
    let mut original = Vec::with_capacity(32);
    let mut lengths = [0u32; 4];
    for (word, (pronounced, ordinal)) in words.iter().zip(&order.ordinals).enumerate() {
        let selection = pronounced.pronunciation().selection();
        let request = selection.request();
        let role = &order.roles[*ordinal];
        if request.source() != order.lexical.tape().source()
            || request.token() != &order.lexical.tape().tokens()[*ordinal]
            || request.analysis() != role.request().analysis()
            || request.basis() != role.request().basis()
            || *selection.candidate_selection().index() != *role.request().choice()
        {
            return Err(ForeignWord { word });
        }
        if pronounced.intent().events().len() > 8 {
            return Err(Capacity);
        }
        lengths[word] = pronounced.intent().events().len() as u32;
        for event in pronounced.intent().events().iter() {
            let SpeechUtteranceIntentEvent::Segment(segment) = event else {
                return Err(ForeignWord { word });
            };
            if original.len() == 32 {
                return Err(Capacity);
            }
            original.push((word, admit_planned_segment_material(segment)?));
        }
    }
    let layout =
        SpeechCompletePhoneLayout4::new(lengths, order.native.clone()).map_err(NativeOrder)?;
    Ok((layout, original))
}

/// Verify every selected word and phone before either synthesis realization.
/// Boundaries remain separate authored events; every segment preserves complete
/// phone, stress, prosody, source and provenance. Only an explicitly retained
/// occurrence rebase may differ from its original checked word intent.
pub fn prepare_speech_plan_coverage<'a, 'word, 'basis>(
    order: &'a PreparedSpeechSpokenOrder<'a>,
    words: &[&'a PreparedPronunciationIntent<'word, 'basis>],
    intent: &'a SpeechUtteranceIntent,
    witnesses: &'a [SpeechPhoneCompositionWitness],
) -> Result<PreparedSpeechPlanCoverage<'a, 'word, 'basis>, SpeechPlanCoverageRefusal> {
    use SpeechPlanCoverageRefusal::*;
    if intent.language() != order.lexical.tape().source().material().language() {
        return Err(ForeignIntent);
    }
    let (layout, original) = validated_words(order, words)?;
    let mut phone_events = Vec::with_capacity(32);
    if intent.events().len() > crate::MAXIMUM_EVENTS {
        return Err(Capacity);
    }
    let mut segments = Vec::with_capacity(32);
    for (event, value) in intent.events().iter().enumerate() {
        if let SpeechUtteranceIntentEvent::Segment(segment) = value {
            if segments.len() == 32 {
                return Err(Capacity);
            }
            segments.push((event, admit_planned_segment_material(segment)?));
        }
    }
    if segments.len() != original.len() || witnesses.len() != original.len() {
        return Err(PhoneCount);
    }
    let sequence = segments
        .first()
        .map(|(_, segment)| segment.occurrence().sequence_id());
    for (index, (((event, current), (word, old)), witness)) in
        segments.iter().zip(original).zip(witnesses).enumerate()
    {
        let occurrence = current.occurrence();
        if witness.original() != &old
            || witness.composite() != current
            || witness.layout() != &layout
            || *witness.word_position() as usize != word
            || *witness.global_ordinal() as usize != index
            || witness.target().utterance_id() != intent.utterance_id()
            || witness.target().revision_id() != intent.revision_id()
            || witness.target().inventory_id() != intent.inventory_id()
            || witness.target().language() != intent.language()
            || occurrence.utterance_id() != intent.utterance_id()
            || occurrence.revision_id() != intent.revision_id()
            || occurrence.inventory_id() != intent.inventory_id()
            || occurrence.language() != intent.language()
            || Some(occurrence.sequence_id()) != sequence
            || *occurrence.ordinal() as usize != index
        {
            return Err(Occurrence { event: *event });
        }
        if current.phone() != old.phone()
            || current.phoneme() != old.phoneme()
            || current.stress() != old.stress()
            || current.word_position() != old.word_position()
            || current.prosody() != old.prosody()
            || current.provenance() != old.provenance()
            || current.sources() != old.sources()
        {
            return Err(Phone { event: *event });
        }
        phone_events.push(*event);
    }
    Ok(PreparedSpeechPlanCoverage {
        order,
        words: words.to_vec(),
        intent,
        layout,
        witnesses,
        phone_events,
    })
}
