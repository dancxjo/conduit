//! Shared finite selection algorithm with caller-owned result slots.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BorrowedContextOmission {
    pub candidate_index: usize,
    pub reason: ContextOmissionReason,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BorrowedContextSelectionDisposition {
    Complete,
    Omitted,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BorrowedContextSelection {
    pub selected_len: usize,
    pub omitted_len: usize,
    pub used: SelectedContextCost,
    pub rationale: SelectedContextRationale,
    pub disposition: BorrowedContextSelectionDisposition,
}
pub struct PreparedContextSelector {
    order: Vec<usize>,
    maximum_candidates: usize,
}
impl PreparedContextSelector {
    pub fn prepare(maximum_candidates: usize) -> Result<Self, ContextSelectionRefusal> {
        if maximum_candidates == 0 || maximum_candidates > MAXIMUM_HYBRID_OUTPUT_CANDIDATES as usize
        {
            return Err(ContextSelectionRefusal::InvalidBound);
        }
        Ok(Self {
            order: Vec::with_capacity(maximum_candidates),
            maximum_candidates,
        })
    }
    /// Selected indices and omission slots are caller-owned. Failure clears both.
    /// No source access, owned cloning, ranking inference or allocation occurs.
    pub fn select(
        &mut self,
        policy: &ContextSelectionPolicy,
        candidates: &[BorrowedContextCandidate<'_>],
        selected: &mut [usize],
        omitted: &mut [Option<BorrowedContextOmission>],
    ) -> Result<BorrowedContextSelection, ContextSelectionRefusal> {
        selected.fill(usize::MAX);
        omitted.fill(None);
        self.order.clear();
        let result = self.execute(policy, candidates, selected, omitted);
        if result.is_err() {
            selected.fill(usize::MAX);
            omitted.fill(None);
        }
        result
    }
    fn execute(
        &mut self,
        policy: &ContextSelectionPolicy,
        candidates: &[BorrowedContextCandidate<'_>],
        selected: &mut [usize],
        omitted: &mut [Option<BorrowedContextOmission>],
    ) -> Result<BorrowedContextSelection, ContextSelectionRefusal> {
        if candidates.is_empty() {
            return Err(ContextSelectionRefusal::EmptyCandidates);
        }
        if candidates.len() > MAXIMUM_HYBRID_OUTPUT_CANDIDATES as usize {
            return Err(ContextSelectionRefusal::CandidateLimitExceeded);
        }
        if candidates.len() > self.maximum_candidates
            || selected.len() < candidates.len().min(*policy.maximum_items() as usize)
            || omitted.len() < candidates.len()
        {
            return Err(ContextSelectionRefusal::InvalidBound);
        }
        for (index, candidate) in candidates.iter().enumerate() {
            validate_identity(candidate.reranking_policy_identity)?;
            candidate.chunk.validate()?;
            if candidate.token_count == 0 {
                return Err(ContextSelectionRefusal::EmptyTokenCost);
            }
            if candidate.reranked_rank == 0
                || candidate.reranked_rank as usize > candidates.len()
                || candidate.original_rank == 0
                || candidate.contributions.len() == 0
            {
                return Err(ContextSelectionRefusal::InvalidCandidate);
            }
            if candidates[index + 1..].iter().any(|other| {
                other.chunk.identity() == candidate.chunk.identity()
                    || other.reranked_rank == candidate.reranked_rank
            }) {
                return Err(ContextSelectionRefusal::DuplicateCandidate);
            }
            if let Some(group) = candidate.redundancy_group {
                validate_identity(group)?;
            }
            if *policy.ordering() == ContextOrderingPolicy::ChronologicalOldestFirst
                && candidate.temporal.is_none()
            {
                return Err(ContextSelectionRefusal::MissingTemporalEvidence);
            }
            if let Some(temporal) = candidate.temporal {
                temporal.validate(candidate.contributions)?;
            } else if candidate.contributions.any_temporal() {
                return Err(ContextSelectionRefusal::MissingTemporalEvidence);
            }
        }
        self.order.extend(0..candidates.len());
        self.order.sort_unstable_by(|left, right| {
            let (left, right) = (&candidates[*left], &candidates[*right]);
            match policy.ordering() {
                ContextOrderingPolicy::Reranked => left.reranked_rank.cmp(&right.reranked_rank),
                ContextOrderingPolicy::ChronologicalOldestFirst => left
                    .temporal
                    .and_then(|t| t.source_at)
                    .unwrap_or(u64::MAX)
                    .cmp(&right.temporal.and_then(|t| t.source_at).unwrap_or(u64::MAX))
                    .then_with(|| left.reranked_rank.cmp(&right.reranked_rank)),
            }
        });
        let mut used = SelectedContextCost {
            bytes: 0,
            tokens: 0,
            work_units: 0,
        };
        let mut selected_len = 0;
        let mut omitted_len = 0;
        for (position, index) in self.order.iter().copied().enumerate() {
            let candidate = &candidates[index];
            if *policy.redundancy() == ContextRedundancyPolicy::OnePerReviewedGroup {
                let group = candidate
                    .redundancy_group
                    .ok_or(ContextSelectionRefusal::MissingRedundancyGroup)?;
                // As in the existing owner, encountering a group consumes it even when
                // that first candidate subsequently fails a byte/token/work/item budget.
                if self.order[..position]
                    .iter()
                    .any(|prior| candidates[*prior].redundancy_group == Some(group))
                {
                    omitted[omitted_len] = Some(BorrowedContextOmission {
                        candidate_index: index,
                        reason: ContextOmissionReason::ReviewedRedundancy,
                    });
                    omitted_len += 1;
                    continue;
                }
            }
            let cost = SelectedContextCost {
                bytes: candidate.chunk.byte_cost()?,
                tokens: candidate.token_count,
                work_units: 1,
            };
            if let Some(reason) = budget_refusal(policy, &used, &cost, selected_len)? {
                omitted[omitted_len] = Some(BorrowedContextOmission {
                    candidate_index: index,
                    reason,
                });
                omitted_len += 1;
                continue;
            }
            used.bytes += cost.bytes;
            used.tokens += cost.tokens;
            used.work_units += cost.work_units;
            selected[selected_len] = index;
            selected_len += 1;
        }
        if selected_len == 0 {
            return Err(ContextSelectionRefusal::NoSelectedContext);
        }
        Ok(BorrowedContextSelection {
            selected_len,
            omitted_len,
            disposition: if omitted_len == 0 {
                BorrowedContextSelectionDisposition::Complete
            } else {
                BorrowedContextSelectionDisposition::Omitted
            },
            used,
            rationale: match policy.ordering() {
                ContextOrderingPolicy::Reranked => SelectedContextRationale::Reranked,
                ContextOrderingPolicy::ChronologicalOldestFirst => {
                    SelectedContextRationale::TemporalChronology
                }
            },
        })
    }
}
