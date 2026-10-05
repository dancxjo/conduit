//! Finite lifecycle admission and read-only history; domain reduction stays out
//! of core. Commitment never claims external delivery.
use super::event::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RevisionLimits {
    pub history_events: usize,
    pub revisable_units: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RevisionFrontiers<C> {
    pub committed: C,
    pub observed_through: C,
    pub stable_through: Option<C>,
}

pub struct RevisionJournal<'a, D: RevisionDomain> {
    domain: &'a D,
    context: RevisionContext<'a>,
    initial: D::Cursor,
    limits: RevisionLimits,
    frontiers: RevisionFrontiers<D::Cursor>,
    current: Option<RevisionReference<'a>>,
    closed: bool,
    next_sequence: u64,
    events: [Option<&'a RevisionEvent<'a, D>>; MAX_REVISION_HISTORY],
    retained: usize,
    truncated: u64,
}
impl<'a, D: RevisionDomain> RevisionJournal<'a, D> {
    pub fn new(
        domain: &'a D,
        context: RevisionContext<'a>,
        initial: D::Cursor,
        limits: RevisionLimits,
    ) -> Result<Self, RevisionRefusal> {
        if limits.history_events == 0
            || limits.history_events > MAX_REVISION_HISTORY
            || limits.revisable_units == 0
        {
            return Err(RevisionRefusal::Limits);
        }
        if !domain.validate_cursor(initial) {
            return Err(RevisionRefusal::Domain);
        }
        Ok(Self {
            domain,
            context,
            initial,
            limits,
            frontiers: RevisionFrontiers {
                committed: initial,
                observed_through: initial,
                stable_through: None,
            },
            current: None,
            closed: false,
            next_sequence: 1,
            events: [None; MAX_REVISION_HISTORY],
            retained: 0,
            truncated: 0,
        })
    }
    pub fn context(&self) -> RevisionContext<'a> {
        self.context
    }
    pub fn frontiers(&self) -> RevisionFrontiers<D::Cursor> {
        self.frontiers
    }
    pub fn current_proposal(&self) -> Option<RevisionReference<'a>> {
        self.current
    }
    pub fn is_closed(&self) -> bool {
        self.closed
    }
    pub fn truncated_events(&self) -> u64 {
        self.truncated
    }
    pub fn history(&self) -> impl DoubleEndedIterator<Item = &'a RevisionEvent<'a, D>> + '_ {
        self.events[..self.retained].iter().flatten().copied()
    }

    /// All refusal paths leave frontiers, history and current interpretation
    /// unchanged. Identity and explicit edges, never value comparison, govern.
    pub fn append(&mut self, event: &'a RevisionEvent<'a, D>) -> Result<(), RevisionRefusal> {
        validate_change(self.domain, event.change())?;
        let reference = event.reference();
        if reference.context != self.context || event.contract() != self.domain.contract() {
            return Err(RevisionRefusal::Scope);
        }
        if reference.sequence != self.next_sequence {
            return Err(RevisionRefusal::Sequence);
        }
        let next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(RevisionRefusal::SequenceExhausted)?;
        if self
            .history()
            .any(|old| old.reference().event == reference.event)
        {
            return Err(RevisionRefusal::DuplicateIdentity);
        }
        if self.retained == self.limits.history_events {
            return Err(RevisionRefusal::HistoryFull);
        }
        if self.closed && !matches!(event.change(), RevisionChange::Corrected { .. }) {
            return Err(RevisionRefusal::Closed);
        }
        let mut next = self.frontiers;
        let mut current = self.current;
        let mut closed = self.closed;
        match event.change() {
            RevisionChange::Proposed { delta } => {
                if current.is_some() {
                    return Err(RevisionRefusal::ProposalAlreadyActive);
                }
                self.admit_revision(delta, &mut next)?;
                current = Some(reference);
            }
            RevisionChange::Revised { replaces, delta } => {
                self.require_current(*replaces)?;
                self.admit_revision(delta, &mut next)?;
                current = Some(reference);
            }
            RevisionChange::Stable { revision, through } => {
                self.require_current(*revision)?;
                self.admit_frontier(*through)?;
                if next.stable_through.is_some_and(|old| *through < old) {
                    return Err(RevisionRefusal::Frontier);
                }
                next.stable_through = Some(*through);
            }
            RevisionChange::Committed { revision, through } => {
                self.require_current(*revision)?;
                self.admit_frontier(*through)?;
                if *through <= next.committed {
                    return Err(RevisionRefusal::Frontier);
                }
                next.committed = *through;
            }
            RevisionChange::Withdrawn {
                revision, delta, ..
            } => {
                self.require_current(*revision)?;
                if self.domain.region(delta).1 > next.observed_through {
                    return Err(RevisionRefusal::Frontier);
                }
                self.admit_revision(delta, &mut next)?;
                current = None;
            }
            RevisionChange::Corrected { commit, delta, .. } => {
                let target = self
                    .history()
                    .find(|old| old.reference() == *commit)
                    .ok_or(RevisionRefusal::UnknownCommit)?;
                let RevisionChange::Committed { through, .. } = target.change() else {
                    return Err(RevisionRefusal::UnknownCommit);
                };
                let (start, end) = self.domain.region(delta);
                if start < self.initial || end > *through || end > next.committed {
                    return Err(RevisionRefusal::CorrectionOutsideCommit);
                }
                if next.stable_through.is_some_and(|through| start < through) {
                    next.stable_through = None;
                }
                // Preserve the old assertion and commitment cursor. Corrected
                // truth does not inherit stale stability evidence or reopen
                // ordinary revision after closure.
            }
            RevisionChange::Closed => {
                closed = true;
            }
        }
        self.events[self.retained] = Some(event);
        self.retained += 1;
        self.next_sequence = next_sequence;
        self.frontiers = next;
        self.current = current;
        self.closed = closed;
        Ok(())
    }

    fn require_current(&self, reference: RevisionReference<'a>) -> Result<(), RevisionRefusal> {
        match self.current {
            None => Err(RevisionRefusal::NoActiveProposal),
            Some(current) if current != reference => Err(RevisionRefusal::StaleRevision),
            Some(_) => Ok(()),
        }
    }
    fn admit_frontier(&self, through: D::Cursor) -> Result<(), RevisionRefusal> {
        if through < self.frontiers.committed || through > self.frontiers.observed_through {
            return Err(RevisionRefusal::Frontier);
        }
        Ok(())
    }
    fn admit_revision(
        &self,
        delta: &D::Delta,
        next: &mut RevisionFrontiers<D::Cursor>,
    ) -> Result<(), RevisionRefusal> {
        let (start, end) = self.domain.region(delta);
        if start < next.committed {
            return Err(RevisionRefusal::CommittedHistoryRequiresCorrection);
        }
        let observed = core::cmp::max(next.observed_through, end);
        let units = self
            .domain
            .distance(next.committed, observed)
            .ok_or(RevisionRefusal::Domain)?;
        if units > self.limits.revisable_units {
            return Err(RevisionRefusal::RevisableRegionBound);
        }
        if next.stable_through.is_some_and(|through| start < through) {
            next.stable_through = None;
        }
        next.observed_through = observed;
        Ok(())
    }

    /// Explicitly discard only a superseded, uncommitted prefix. Commit records
    /// and their original interpretation remain pinned within the admitted
    /// capacity. Pressure refuses rather than silently erasing committed truth.
    pub fn truncate_prefix(&mut self, count: usize) -> Result<(), RevisionRefusal> {
        if count == 0 || count > self.retained {
            return Err(RevisionRefusal::InvalidTruncation);
        }
        let prefix = &self.events[..count];
        for old in prefix.iter().flatten() {
            if self.current == Some(old.reference()) {
                return Err(RevisionRefusal::TruncationWouldEraseCurrentRevision);
            }
            // Earlier deltas can contribute to a later committed view even
            // when the commit names only the latest revision. Pin the entire
            // retained epoch once any portion commits, including corrections.
            if self
                .history()
                .any(|event| matches!(event.change(), RevisionChange::Committed { .. }))
            {
                return Err(RevisionRefusal::TruncationWouldEraseCommittedHistory);
            }
        }
        let truncated = self
            .truncated
            .checked_add(count as u64)
            .ok_or(RevisionRefusal::InvalidTruncation)?;
        self.events.copy_within(count..self.retained, 0);
        self.events[self.retained - count..self.retained].fill(None);
        self.retained -= count;
        self.truncated = truncated;
        Ok(())
    }

    pub fn replay(
        domain: &'a D,
        context: RevisionContext<'a>,
        initial: D::Cursor,
        limits: RevisionLimits,
        history: &[&'a RevisionEvent<'a, D>],
    ) -> Result<Self, RevisionRefusal> {
        if history.len() > limits.history_events {
            return Err(RevisionRefusal::HistoryFull);
        }
        if history
            .first()
            .is_some_and(|event| event.reference().sequence != 1)
        {
            return Err(RevisionRefusal::TruncatedHistory);
        }
        let mut journal = Self::new(domain, context, initial, limits)?;
        for event in history {
            journal.append(event)?;
        }
        Ok(journal)
    }
}
