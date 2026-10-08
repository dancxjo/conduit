//! Root-SDK constant-pitch basis. The actual committed owner stays borrowed;
//! no trajectory or temporal approximation is manufactured for unchanged events.
use super::owner::PreparedCommittedWordPitch;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::semantic::{
    SpeechCycleSpecification, SpeechFundamentalCycle, SpeechUtteranceIntentEvent,
};

#[derive(Debug)]
pub enum Refusal {
    Segment,
    ChangedOriginal,
    VaryingPitch,
    UnsupportedSpecification,
    ForeignOwner,
    Native,
}
pub struct CommittedConstantBasis<'o, 'a, 'coverage, 'word, 'basis, 'commit> {
    owner: &'o PreparedCommittedWordPitch<'a, 'coverage, 'word, 'basis, 'commit>,
    ordinal: usize,
    original_event: Vec<u8>,
    original_cycle: Vec<u8>,
}
impl<'o, 'a, 'coverage, 'word, 'basis, 'commit>
    CommittedConstantBasis<'o, 'a, 'coverage, 'word, 'basis, 'commit>
{
    pub fn prepare(
        owner: &'o PreparedCommittedWordPitch<'a, 'coverage, 'word, 'basis, 'commit>,
        ordinal: usize,
    ) -> Result<Self, Refusal> {
        let segment_at = |intent: &conduit_speech::semantic::SpeechUtteranceIntent| {
            intent
                .events()
                .iter()
                .filter(|event| matches!(event, SpeechUtteranceIntentEvent::Segment(_)))
                .nth(ordinal)
                .cloned()
        };
        let original = segment_at(owner.original().coverage().intent()).ok_or(Refusal::Segment)?;
        let realized = segment_at(owner.realized()).ok_or(Refusal::Segment)?;
        if original != realized {
            return Err(Refusal::ChangedOriginal);
        }
        let SpeechUtteranceIntentEvent::Segment(segment) = &original else {
            return Err(Refusal::Segment);
        };
        if owner
            .segment_pitch()
            .iter()
            .any(|pitch| pitch.segment().occurrence() == segment.occurrence())
        {
            return Err(Refusal::VaryingPitch);
        }
        let SpeechCycleSpecification::Known(cycle) = segment.prosody().fundamental_cycle() else {
            return Err(Refusal::UnsupportedSpecification);
        };
        Ok(Self {
            owner,
            ordinal,
            original_event: original.clone().encode().map_err(|_| Refusal::Native)?,
            original_cycle: SpeechFundamentalCycle::new(
                *cycle.denominator(),
                *cycle.numerator_seconds(),
            )
            .map_err(|_| Refusal::Native)?
            .encode()
            .map_err(|_| Refusal::Native)?,
        })
    }
    pub fn require_owner(
        &self,
        offered: &PreparedCommittedWordPitch<'a, 'coverage, 'word, 'basis, 'commit>,
    ) -> Result<(), Refusal> {
        if core::ptr::eq(self.owner, offered) {
            Ok(())
        } else {
            Err(Refusal::ForeignOwner)
        }
    }
    pub fn ordinal(&self) -> usize {
        self.ordinal
    }
    pub fn original_event(&self) -> &[u8] {
        &self.original_event
    }
    pub fn original_cycle(&self) -> &[u8] {
        &self.original_cycle
    }
    pub fn owner(&self) -> &'o PreparedCommittedWordPitch<'a, 'coverage, 'word, 'basis, 'commit> {
        self.owner
    }
}
