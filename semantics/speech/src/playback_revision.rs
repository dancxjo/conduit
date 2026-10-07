//! Whole native utterance epoch under shared revision law. Units are neither
//! text scalars nor PCM frames. First evidenced playback freezes interpretation;
//! later correction preserves the old native acknowledgement and source facts.
use crate::{
    native_playback_back::{NativeSpeechPlaybackBack, PlayedTapeEvidence},
    playback_basis::PreparedSpeechPlaybackTape,
    semantic::*,
};
use alloc::boxed::Box;
use conduit_core::revision::*;
use conduit_plot::rust_binding::NativeBindingRefusal;
use core::cmp::Ordering;

#[derive(Clone, Copy)]
pub struct PlaybackEpochCursor<'a> {
    unit: u64,
    played: Option<PlayedTapeEvidence<'a, 'a>>,
}
impl<'a> PlaybackEpochCursor<'a> {
    pub fn initial() -> Self {
        Self {
            unit: 0,
            played: None,
        }
    }
    pub fn interpreted() -> Self {
        Self {
            unit: 1,
            played: None,
        }
    }
    pub fn from_played(evidence: PlayedTapeEvidence<'a, 'a>) -> Self {
        Self {
            unit: 1,
            played: Some(evidence),
        }
    }
    pub fn acknowledgement(&self) -> Option<&'a SpeechPlaybackAcknowledgement> {
        self.played.map(|e| e.acknowledgement())
    }
}
// One interpretation epoch is frozen by its first irreversible audible frame;
// receipt payload preserves that effect evidence without changing cursor units.
impl PartialEq for PlaybackEpochCursor<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.unit == other.unit
    }
}
impl Eq for PlaybackEpochCursor<'_> {}
impl PartialOrd for PlaybackEpochCursor<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for PlaybackEpochCursor<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.unit.cmp(&other.unit)
    }
}

pub struct PreparedPlaybackChange<'a> {
    data: PlaybackChangeData,
    old: Option<&'a PreparedSpeechPlaybackTape<'a>>,
    next: Option<&'a PreparedSpeechPlaybackTape<'a>>,
}
enum PlaybackChangeData {
    Interpretation(Box<SpeechPlaybackInterpretationChange>),
    Withdrawal(Box<SpeechPlaybackWithdrawal>),
    Correction(Box<SpeechPlaybackCorrection>),
}
impl<'a> PreparedPlaybackChange<'a> {
    pub fn interpretation(
        old: Option<&'a PreparedSpeechPlaybackTape<'a>>,
        next: &'a PreparedSpeechPlaybackTape<'a>,
    ) -> Result<Self, NativeBindingRefusal> {
        let data = SpeechPlaybackInterpretationChange::new(
            next.basis().clone(),
            old.map(|tape| tape.basis().clone()),
        )?;
        Ok(Self {
            data: PlaybackChangeData::Interpretation(Box::new(data)),
            old,
            next: Some(next),
        })
    }
    pub fn withdrawal(
        old: &'a PreparedSpeechPlaybackTape<'a>,
    ) -> Result<Self, NativeBindingRefusal> {
        Ok(Self {
            data: PlaybackChangeData::Withdrawal(Box::new(SpeechPlaybackWithdrawal::new(
                old.basis().clone(),
            )?)),
            old: Some(old),
            next: None,
        })
    }
    pub fn correction(
        old: &'a PreparedSpeechPlaybackTape<'a>,
        next: &'a PreparedSpeechPlaybackTape<'a>,
    ) -> Result<Self, NativeBindingRefusal> {
        Ok(Self {
            data: PlaybackChangeData::Correction(Box::new(SpeechPlaybackCorrection::new(
                old.basis().clone(),
                next.basis().clone(),
            )?)),
            old: Some(old),
            next: Some(next),
        })
    }
    pub fn old(&self) -> Option<&'a PreparedSpeechPlaybackTape<'a>> {
        self.old
    }
    pub fn next(&self) -> Option<&'a PreparedSpeechPlaybackTape<'a>> {
        self.next
    }
    pub fn interpretation_data(&self) -> Option<&SpeechPlaybackInterpretationChange> {
        if let PlaybackChangeData::Interpretation(value) = &self.data {
            Some(value)
        } else {
            None
        }
    }
    pub fn correction_data(&self) -> Option<&SpeechPlaybackCorrection> {
        if let PlaybackChangeData::Correction(value) = &self.data {
            Some(value)
        } else {
            None
        }
    }
}
pub struct PlaybackRevisionDomain<'a> {
    pub subject: &'a SpeechUtteranceId,
}
impl<'a> RevisionDomain for PlaybackRevisionDomain<'a> {
    type Delta = PreparedPlaybackChange<'a>;
    type Cursor = PlaybackEpochCursor<'a>;
    fn contract(&self) -> RevisionText<'_> {
        RevisionText::new("speech/playback-utterance-epoch@1").expect("constant")
    }
    fn validate_delta(&self, role: RevisionDeltaRole, delta: &Self::Delta) -> bool {
        let belongs =
            |tape: &PreparedSpeechPlaybackTape<'_>| tape.source().utterance_id() == self.subject;
        match (role, &delta.data) {
            (RevisionDeltaRole::Proposal, PlaybackChangeData::Interpretation(value)) => {
                delta.old.is_none() && delta.next.is_some_and(belongs) && value.prior().is_none()
            }
            (RevisionDeltaRole::Revision, PlaybackChangeData::Interpretation(_)) => {
                delta.old.is_some_and(belongs) && delta.next.is_some_and(belongs)
            }
            (RevisionDeltaRole::Withdrawal, PlaybackChangeData::Withdrawal(value)) => {
                delta.old.is_some_and(belongs)
                    && value.basis().intent().utterance_id() == self.subject
            }
            (RevisionDeltaRole::Correction, PlaybackChangeData::Correction(_)) => {
                delta.old.is_some_and(belongs) && delta.next.is_some()
            }
            _ => false,
        }
    }
    fn validate_cursor(&self, cursor: Self::Cursor) -> bool {
        cursor.unit <= 1
            && cursor.played.is_none_or(|proof| {
                proof.tape().source().utterance_id() == self.subject
                    && *proof.acknowledgement().disposition() == SpeechPlaybackDisposition::Played
            })
    }
    fn region(&self, _: &Self::Delta) -> (Self::Cursor, Self::Cursor) {
        (
            PlaybackEpochCursor::initial(),
            PlaybackEpochCursor::interpreted(),
        )
    }
    fn distance(&self, start: Self::Cursor, end: Self::Cursor) -> Option<u64> {
        end.unit.checked_sub(start.unit)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackRevisionRefusal {
    Journal(RevisionRefusal),
    Source,
    QueuedHistory,
    PlayedEvidence,
}
pub struct PlaybackRevisionJournal<'a> {
    journal: RevisionJournal<'a, PlaybackRevisionDomain<'a>>,
    current: Option<&'a PreparedSpeechPlaybackTape<'a>>,
    producer: Option<NativeSpeechPlaybackBack<'a>>,
    needs_replan: bool,
    cancelled: bool,
    latest_played: Option<&'a SpeechPlaybackAcknowledgement>,
}
impl<'a> PlaybackRevisionJournal<'a> {
    pub fn new(
        domain: &'a PlaybackRevisionDomain<'a>,
        context: RevisionContext<'a>,
        limits: RevisionLimits,
        producer: NativeSpeechPlaybackBack<'a>,
    ) -> Result<Self, RevisionRefusal> {
        if context.subject.as_str() != domain.subject.get().as_str()
            || producer.queued_frames() != 0
            || producer.is_cancelled()
            || producer.tape().source().utterance_id() != domain.subject
        {
            return Err(RevisionRefusal::Domain);
        }
        Ok(Self {
            journal: RevisionJournal::new(domain, context, PlaybackEpochCursor::initial(), limits)?,
            current: None,
            producer: Some(producer),
            needs_replan: false,
            cancelled: false,
            latest_played: None,
        })
    }
    pub fn journal(&self) -> &RevisionJournal<'a, PlaybackRevisionDomain<'a>> {
        &self.journal
    }
    pub fn requires_replan(&self) -> bool {
        self.needs_replan
    }
    /// Call only while preparing a new checked Plan. The scheduled owner has no
    /// mutable access to this journal. A changed basis never inherits its old
    /// Gear activation; native preparation rechecks the exact new configuration.
    pub fn rebind_plan<const PORTS: usize>(
        &mut self,
        gear: &conduit_core::PlannedGear,
    ) -> Result<(), crate::native_playback_back::NativePlaybackPreparationRefusal> {
        use crate::native_playback_back::NativePlaybackPreparationRefusal as Refusal;
        if self.cancelled || !self.needs_replan {
            return Err(Refusal::Identity);
        }
        let tape = self.current.ok_or(Refusal::Identity)?;
        if self
            .producer
            .as_ref()
            .is_none_or(|back| back.queued_frames() != 0 || !back.tape().same_snapshot(tape))
        {
            return Err(Refusal::Identity);
        }
        let producer = NativeSpeechPlaybackBack::prepare::<PORTS>(
            gear,
            tape,
            conduit_kernel::PortId(0),
            conduit_kernel::PortId(0),
        )?;
        self.producer = Some(producer);
        self.needs_replan = false;
        Ok(())
    }
    pub fn producer(&self) -> Option<&NativeSpeechPlaybackBack<'a>> {
        self.producer.as_ref()
    }
    pub fn acknowledge(
        &mut self,
        ack: &'a SpeechPlaybackAcknowledgement,
    ) -> Result<
        Option<PlayedTapeEvidence<'a, 'a>>,
        crate::native_playback_back::PlaybackAcknowledgementRefusal,
    > {
        let evidence = self
            .producer
            .as_mut()
            .ok_or(crate::native_playback_back::PlaybackAcknowledgementRefusal::Basis)?
            .acknowledge(ack)?;
        if evidence.is_some() {
            self.latest_played = Some(ack);
        }
        Ok(evidence)
    }
    /// Source reduction supplements core lifecycle law. A queued old tape stays
    /// effect-owned; ordinary revision/withdrawal cannot pretend it was removed.
    pub fn append(
        &mut self,
        event: &'a RevisionEvent<'a, PlaybackRevisionDomain<'a>>,
        replacement: Option<NativeSpeechPlaybackBack<'a>>,
    ) -> Result<(), PlaybackRevisionRefusal> {
        if replacement.is_some()
            && !matches!(
                event.change(),
                RevisionChange::Proposed { .. } | RevisionChange::Revised { .. }
            )
        {
            return Err(PlaybackRevisionRefusal::Source);
        }
        let check_current = |old: Option<&PreparedSpeechPlaybackTape<'_>>| {
            self.current
                .zip(old)
                .is_some_and(|(lhs, rhs)| lhs.same_snapshot(rhs))
        };
        let next = match event.change() {
            RevisionChange::Proposed { delta } => {
                if !delta
                    .next
                    .zip(replacement.as_ref().or(self.producer.as_ref()))
                    .is_some_and(|(next, back)| {
                        next.same_snapshot(back.tape())
                            && back.queued_frames() == 0
                            && !back.is_cancelled()
                    })
                {
                    return Err(PlaybackRevisionRefusal::Source);
                }
                delta.next
            }
            RevisionChange::Revised { delta, .. } | RevisionChange::Withdrawn { delta, .. } => {
                if !check_current(delta.old) {
                    return Err(PlaybackRevisionRefusal::Source);
                }
                let back = self
                    .producer
                    .as_ref()
                    .ok_or(PlaybackRevisionRefusal::Source)?;
                if !delta.old.is_some_and(|old| old.same_snapshot(back.tape())) {
                    return Err(PlaybackRevisionRefusal::Source);
                }
                if back.queued_frames() != 0 {
                    return Err(PlaybackRevisionRefusal::QueuedHistory);
                }
                match event.change() {
                    RevisionChange::Revised { .. } => {
                        if !delta
                            .next
                            .zip(replacement.as_ref())
                            .is_some_and(|(next, back)| {
                                next.same_snapshot(back.tape())
                                    && back.queued_frames() == 0
                                    && !back.is_cancelled()
                            })
                        {
                            return Err(PlaybackRevisionRefusal::Source);
                        }
                    }
                    RevisionChange::Withdrawn { .. } => {
                        if replacement.is_some() {
                            return Err(PlaybackRevisionRefusal::Source);
                        }
                    }
                    _ => unreachable!(),
                }
                delta.next
            }
            RevisionChange::Committed { through, .. } => {
                let proof = through
                    .played
                    .ok_or(PlaybackRevisionRefusal::PlayedEvidence)?;
                if !check_current(Some(proof.tape())) {
                    return Err(PlaybackRevisionRefusal::Source);
                }
                if !self
                    .latest_played
                    .is_some_and(|ack| ack == proof.acknowledgement())
                    || !self.producer.as_ref().is_some_and(|back| {
                        back.played_frames() >= *proof.acknowledgement().through_frame()
                    })
                {
                    return Err(PlaybackRevisionRefusal::PlayedEvidence);
                }
                self.current
            }
            RevisionChange::Corrected { commit, delta, .. } => {
                let old = self
                    .journal
                    .history()
                    .find(|old| old.reference() == *commit)
                    .ok_or(PlaybackRevisionRefusal::Journal(
                        RevisionRefusal::UnknownCommit,
                    ))?;
                let RevisionChange::Committed { revision, through } = old.change() else {
                    return Err(PlaybackRevisionRefusal::Journal(
                        RevisionRefusal::UnknownCommit,
                    ));
                };
                let proof = through
                    .played
                    .ok_or(PlaybackRevisionRefusal::PlayedEvidence)?;
                if !delta.old.is_some_and(|old| old.same_snapshot(proof.tape())) {
                    return Err(PlaybackRevisionRefusal::Source);
                }
                let origin = self
                    .journal
                    .history()
                    .find(|old| old.reference() == *revision)
                    .ok_or(PlaybackRevisionRefusal::Source)?;
                let previous = match origin.change() {
                    RevisionChange::Proposed { delta } | RevisionChange::Revised { delta, .. } => {
                        delta.next
                    }
                    _ => None,
                };
                if !previous
                    .zip(delta.old)
                    .is_some_and(|(lhs, rhs)| lhs.same_snapshot(rhs))
                {
                    return Err(PlaybackRevisionRefusal::Source);
                }
                self.current // corrected truth never silently reopens old playback
            }
            _ => self.current,
        };
        self.journal
            .append(event)
            .map_err(PlaybackRevisionRefusal::Journal)?;
        match event.change() {
            RevisionChange::Proposed { .. } if replacement.is_some() => {
                self.producer = replacement;
                self.needs_replan = true;
                self.latest_played = None;
            }
            RevisionChange::Revised { .. } => {
                if let Some(back) = self.producer.as_mut() {
                    conduit_kernel::scheduler::StepBack::<1>::cancel(back);
                }
                self.producer = replacement;
                self.needs_replan = true;
                self.latest_played = None;
            }
            RevisionChange::Withdrawn { .. } => {
                if let Some(back) = self.producer.as_mut() {
                    conduit_kernel::scheduler::StepBack::<1>::cancel(back);
                }
                self.producer = None;
                self.latest_played = None;
            }
            _ => {}
        }
        self.current = next;
        Ok(())
    }
}

// One epoch owns its producer: callers cannot replace a queued producer with a
// fresh duplicate to manufacture preplay state. Play delegates without growth.
impl<const PORTS: usize> conduit_kernel::scheduler::StepBack<PORTS>
    for PlaybackRevisionJournal<'_>
{
    fn step(
        &mut self,
        io: &mut conduit_kernel::scheduler::StepIo<PORTS>,
        bytes: &conduit_kernel::scheduler::StepInputBytes<'_, PORTS>,
    ) -> conduit_kernel::scheduler::StepOutcome {
        if self.cancelled {
            return conduit_kernel::scheduler::StepOutcome::Complete;
        }
        if self.needs_replan {
            return conduit_kernel::scheduler::StepOutcome::Await;
        }
        if self.current.is_none() {
            return conduit_kernel::scheduler::StepOutcome::Await;
        }
        self.producer
            .as_mut()
            .map_or(conduit_kernel::scheduler::StepOutcome::Complete, |back| {
                back.step(io, bytes)
            })
    }
    fn step_committed(&mut self) {
        if let Some(back) = self.producer.as_mut() {
            conduit_kernel::scheduler::StepBack::<PORTS>::step_committed(back);
        }
    }
    fn prepared_output(&self, port: conduit_kernel::PortId) -> Option<&[u8]> {
        self.producer.as_ref().and_then(|back| {
            conduit_kernel::scheduler::StepBack::<PORTS>::prepared_output(back, port)
        })
    }
    fn cancel(&mut self) {
        self.cancelled = true;
        if let Some(back) = self.producer.as_mut() {
            conduit_kernel::scheduler::StepBack::<PORTS>::cancel(back);
        }
    }
}
