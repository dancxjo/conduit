//! Exact parser-commit custody for an already admitted complete speech intent.
//! This is a custody adapter over the existing intent, not a linguistic IR or
//! acoustic projector. It grants no device playback or played-frontier claim.
use crate::{
    committed_token_role::PreparedCommittedTokenRole, plan_coverage::PreparedSpeechPlanCoverage,
};

#[derive(Debug)]
pub enum CommittedPlanCoverageRefusal {
    MissingCommitment,
    ForeignCommitment { word: usize },
}

/// Every spoken word keeps its full Source admission, commit query, exact
/// runtime result and prepared token role alive beside the original coverage.
/// Nonspoken punctuation remains in the complete lexical participation tape.
pub struct PreparedCommittedSpeechPlanCoverage<'a, 'word, 'basis, 'commit> {
    coverage: &'a PreparedSpeechPlanCoverage<'a, 'word, 'basis>,
    commitments: &'a [&'a PreparedCommittedTokenRole<'commit>],
}

impl<'a, 'word, 'basis, 'commit> PreparedCommittedSpeechPlanCoverage<'a, 'word, 'basis, 'commit> {
    pub fn coverage(&self) -> &'a PreparedSpeechPlanCoverage<'a, 'word, 'basis> {
        self.coverage
    }

    pub fn commitments(&self) -> &'a [&'a PreparedCommittedTokenRole<'commit>] {
        self.commitments
    }
}

/// Correlate the already prepared Source role with its original lexical role.
/// Commit legality belongs to the Native admission retained by the role; this
/// adapter compares complete typed material rather than inventing a frontier
/// predicate or reconstructing authority from an ordinal or display IPA.
pub fn prepare_committed_speech_plan_coverage<'a, 'word, 'basis, 'commit>(
    coverage: &'a PreparedSpeechPlanCoverage<'a, 'word, 'basis>,
    commitments: &'a [&'a PreparedCommittedTokenRole<'commit>],
) -> Result<
    PreparedCommittedSpeechPlanCoverage<'a, 'word, 'basis, 'commit>,
    CommittedPlanCoverageRefusal,
> {
    use CommittedPlanCoverageRefusal::*;
    let order = coverage.order();
    if commitments.len() != order.ordinals().len() {
        return Err(MissingCommitment);
    }
    for (word, (commitment, ordinal)) in commitments.iter().zip(order.ordinals()).enumerate() {
        let original = &order.roles()[*ordinal];
        let query = commitment.committed().admission().fact().query();
        if query.beam().lexical().tape() != order.lexical().tape()
            || usize::try_from(*query.dependent()).ok() != Some(*ordinal)
            || commitment.role().request() != original.request()
            || commitment.role().result() != original.result()
        {
            return Err(ForeignCommitment { word });
        }
    }
    Ok(PreparedCommittedSpeechPlanCoverage {
        coverage,
        commitments,
    })
}
