//! Exact committed coverage and rich-prosody custody for a word pitch realization.
use crate::semantic as checked;
use crate::{committed_plan_coverage::PreparedCommittedSpeechPlanCoverage, semantic::*};
use alloc::vec::Vec;
use conduit_language::committed_prosody::PreparedRichProsodyFromCommittedVocative;

#[derive(Debug)]
pub enum CommittedWordPitchRefusal {
    ForeignCommitment,
    Count,
    ForeignSegment,
    ForeignProsody,
    Partition,
    Native,
}

/// Borrows the original complete committed coverage and rich-prosody owner.
/// The new intent is a pitch realization; its original pronunciation coverage
/// remains retained rather than falsely readmitted as unchanged pronunciation.
pub struct PreparedCommittedWordPitch<'a, 'coverage, 'word, 'basis, 'commit> {
    original: &'a PreparedCommittedSpeechPlanCoverage<'coverage, 'word, 'basis, 'commit>,
    rich: &'a PreparedRichProsodyFromCommittedVocative<'a, 'commit>,
    admissions: Vec<checked::SpeechWordPitchRichBasisAdmission>,
    segment_pitch: Vec<SpeechSegmentPitchAdmission>,
    realized: SpeechUtteranceIntent,
}
impl<'a, 'coverage, 'word, 'basis, 'commit>
    PreparedCommittedWordPitch<'a, 'coverage, 'word, 'basis, 'commit>
{
    pub fn original(
        &self,
    ) -> &'a PreparedCommittedSpeechPlanCoverage<'coverage, 'word, 'basis, 'commit> {
        self.original
    }
    pub fn rich(&self) -> &'a PreparedRichProsodyFromCommittedVocative<'a, 'commit> {
        self.rich
    }
    pub fn admissions(&self) -> &[checked::SpeechWordPitchRichBasisAdmission] {
        &self.admissions
    }
    pub fn segment_pitch(&self) -> &[SpeechSegmentPitchAdmission] {
        &self.segment_pitch
    }
    pub fn realized(&self) -> &SpeechUtteranceIntent {
        &self.realized
    }
}

pub fn prepare_committed_word_pitch<'a, 'coverage, 'word, 'basis, 'commit>(
    original: &'a PreparedCommittedSpeechPlanCoverage<'coverage, 'word, 'basis, 'commit>,
    rich: &'a PreparedRichProsodyFromCommittedVocative<'a, 'commit>,
    admissions: &[checked::SpeechWordPitchRichBasisAdmission],
) -> Result<
    PreparedCommittedWordPitch<'a, 'coverage, 'word, 'basis, 'commit>,
    CommittedWordPitchRefusal,
> {
    // Object identity connects the committed discourse to the exact complete
    // coverage commitment; matching a dependent integer alone is insufficient.
    let word_position = original
        .commitments()
        .iter()
        .position(|role| core::ptr::eq(role.committed(), rich.discourse().committed()))
        .ok_or(CommittedWordPitchRefusal::ForeignCommitment)?;
    let coverage = original.coverage();
    let selected = coverage
        .witnesses()
        .iter()
        .zip(coverage.phone_events())
        .filter(|(w, _)| *w.word_position() as usize == word_position)
        .collect::<Vec<_>>();
    if selected.is_empty() || selected.len() != admissions.len() {
        return Err(CommittedWordPitchRefusal::Count);
    }
    let mut events = coverage
        .intent()
        .events()
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    let mut segment_pitch = Vec::with_capacity(admissions.len());
    for (index, ((witness, event), admission)) in selected.iter().zip(admissions).enumerate() {
        let backend = admission.backend();
        let partition = backend.realization().original().partition();
        if partition.original() != witness.composite() {
            return Err(CommittedWordPitchRefusal::ForeignSegment);
        }
        if admission.requested().accepted() != rich.prepared().accepted() {
            return Err(CommittedWordPitchRefusal::ForeignProsody);
        }
        let point = partition.point();
        if index == 0 {
            if *point.offset().numerator_seconds() != 0 {
                return Err(CommittedWordPitchRefusal::Partition);
            }
        } else if admissions[index - 1].backend().end_point() != point {
            return Err(CommittedWordPitchRefusal::Partition);
        }
        if index + 1 == admissions.len()
            && backend.end_point().offset() != point.trajectory().duration()
        {
            return Err(CommittedWordPitchRefusal::Partition);
        }
        let realized = backend.realization().realized();
        let SpeechDurationSpecification::Known(duration) = realized.prosody().duration() else {
            return Err(CommittedWordPitchRefusal::Partition);
        };
        let trajectory = SpeechLinearPitchTrajectory::new(
            SpeechExactDuration::new(*duration.denominator(), *duration.numerator_seconds())
                .map_err(|_| CommittedWordPitchRefusal::Native)?,
            backend.end_cycle().reduced().clone(),
            backend.realization().cycle().reduced().clone(),
        )
        .map_err(|_| CommittedWordPitchRefusal::Native)?;
        segment_pitch.push(
            SpeechSegmentPitchAdmission::new(
                *rich.prepared().accepted().choice().pitch(),
                realized.clone(),
                trajectory,
            )
            .map_err(|_| CommittedWordPitchRefusal::Native)?,
        );
        events[**event] = SpeechUtteranceIntentEvent::segment(
            realized.occurrence().clone(),
            realized.phone().clone(),
            realized.phoneme().clone(),
            realized.prosody().clone(),
            realized.provenance().clone(),
            realized.sources().clone(),
            realized.stress().clone(),
            realized.word_position().clone(),
        )
        .map_err(|_| CommittedWordPitchRefusal::Native)?;
    }
    let intent = coverage.intent();
    let realized = SpeechUtteranceIntent::new(
        conduit_plot::rust_binding::BoundedSequence::try_from_iter(events)
            .map_err(|_| CommittedWordPitchRefusal::Native)?,
        intent.inventory_id().clone(),
        intent.language().clone(),
        intent.provenance().clone(),
        intent.revision_id().clone(),
        intent.utterance_id().clone(),
    )
    .map_err(|_| CommittedWordPitchRefusal::Native)?;
    Ok(PreparedCommittedWordPitch {
        original,
        rich,
        admissions: admissions.to_vec(),
        segment_pitch,
        realized,
    })
}
