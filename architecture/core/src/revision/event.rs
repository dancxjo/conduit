//! Immutable typed events and exact correlation for one revisable subject.
pub const MAX_REVISION_TEXT_BYTES: usize = 192;
pub const MAX_REVISION_EVIDENCE: usize = 8;
pub const MAX_REVISION_HISTORY: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RevisionText<'a>(&'a str);
impl<'a> RevisionText<'a> {
    pub const fn new(value: &'a str) -> Result<Self, RevisionRefusal> {
        if value.is_empty() || value.len() > MAX_REVISION_TEXT_BYTES {
            return Err(RevisionRefusal::TextBound);
        }
        Ok(Self(value))
    }
    pub fn as_str(self) -> &'a str {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RevisionContext<'a> {
    pub stream: RevisionText<'a>,
    pub subject: RevisionText<'a>,
    pub epoch: RevisionText<'a>,
    pub producer: RevisionText<'a>,
    pub policy: RevisionText<'a>,
}

/// Full identity includes scope and sequence; the label alone is not identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RevisionReference<'a> {
    pub context: RevisionContext<'a>,
    pub sequence: u64,
    pub event: RevisionText<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RevisionEvidence<'a> {
    pub source: RevisionText<'a>,
    pub generation: RevisionText<'a>,
}

/// Domains own cursor units, finite delta representation and interpretation.
/// Methods must be deterministic and bounded under the named contract. This
/// library admits semantic data, not hostile-code confinement or execution fuel.
pub trait RevisionDomain {
    type Delta;
    type Cursor: Copy + Ord;
    fn contract(&self) -> RevisionText<'_>;
    fn validate_delta(&self, role: RevisionDeltaRole, delta: &Self::Delta) -> bool;
    fn validate_cursor(&self, cursor: Self::Cursor) -> bool;
    fn region(&self, delta: &Self::Delta) -> (Self::Cursor, Self::Cursor);
    fn distance(&self, start: Self::Cursor, end: Self::Cursor) -> Option<u64>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevisionDeltaRole {
    Proposal,
    Revision,
    Withdrawal,
    Correction,
}

pub enum RevisionChange<'a, T, C> {
    Proposed {
        delta: &'a T,
    },
    Revised {
        replaces: RevisionReference<'a>,
        delta: &'a T,
    },
    Stable {
        revision: RevisionReference<'a>,
        through: C,
    },
    Committed {
        revision: RevisionReference<'a>,
        through: C,
    },
    Withdrawn {
        revision: RevisionReference<'a>,
        delta: &'a T,
        reason: RevisionText<'a>,
    },
    Corrected {
        commit: RevisionReference<'a>,
        delta: &'a T,
        reason: RevisionText<'a>,
    },
    Closed,
}

pub struct RevisionEvent<'a, D: RevisionDomain> {
    reference: RevisionReference<'a>,
    contract: RevisionText<'a>,
    evidence: &'a [RevisionEvidence<'a>],
    change: RevisionChange<'a, D::Delta, D::Cursor>,
}
impl<'a, D: RevisionDomain> RevisionEvent<'a, D> {
    pub fn new(
        domain: &'a D,
        reference: RevisionReference<'a>,
        evidence: &'a [RevisionEvidence<'a>],
        change: RevisionChange<'a, D::Delta, D::Cursor>,
    ) -> Result<Self, RevisionRefusal> {
        if reference.sequence == 0 {
            return Err(RevisionRefusal::Sequence);
        }
        if evidence.is_empty() || evidence.len() > MAX_REVISION_EVIDENCE {
            return Err(RevisionRefusal::EvidenceBound);
        }
        for (index, item) in evidence.iter().enumerate() {
            if evidence[..index].contains(item) {
                return Err(RevisionRefusal::DuplicateEvidence);
            }
        }
        validate_change(domain, &change)?;
        Ok(Self {
            reference,
            contract: domain.contract(),
            evidence,
            change,
        })
    }
    pub fn reference(&self) -> RevisionReference<'a> {
        self.reference
    }
    pub fn contract(&self) -> RevisionText<'a> {
        self.contract
    }
    pub fn evidence(&self) -> &'a [RevisionEvidence<'a>] {
        self.evidence
    }
    pub fn change(&self) -> &RevisionChange<'a, D::Delta, D::Cursor> {
        &self.change
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevisionRefusal {
    TextBound,
    EvidenceBound,
    DuplicateEvidence,
    Domain,
    Limits,
    Scope,
    Sequence,
    SequenceExhausted,
    DuplicateIdentity,
    HistoryFull,
    ProposalAlreadyActive,
    NoActiveProposal,
    StaleRevision,
    Frontier,
    CommittedHistoryRequiresCorrection,
    UnknownCommit,
    CorrectionOutsideCommit,
    Closed,
    RevisableRegionBound,
    TruncationWouldEraseCommittedHistory,
    TruncationWouldEraseCurrentRevision,
    InvalidTruncation,
    TruncatedHistory,
}

pub(super) fn validate_change<D: RevisionDomain>(
    domain: &D,
    change: &RevisionChange<'_, D::Delta, D::Cursor>,
) -> Result<(), RevisionRefusal> {
    let (role, delta) = match change {
        RevisionChange::Proposed { delta } => (RevisionDeltaRole::Proposal, *delta),
        RevisionChange::Revised { delta, .. } => (RevisionDeltaRole::Revision, *delta),
        RevisionChange::Withdrawn { delta, .. } => (RevisionDeltaRole::Withdrawal, *delta),
        RevisionChange::Corrected { delta, .. } => (RevisionDeltaRole::Correction, *delta),
        RevisionChange::Stable { through, .. } | RevisionChange::Committed { through, .. } => {
            return if domain.validate_cursor(*through) {
                Ok(())
            } else {
                Err(RevisionRefusal::Domain)
            };
        }
        RevisionChange::Closed => return Ok(()),
    };
    if !domain.validate_delta(role, delta) {
        return Err(RevisionRefusal::Domain);
    }
    let (start, end) = domain.region(delta);
    if !domain.validate_cursor(start) || !domain.validate_cursor(end) || start >= end {
        return Err(RevisionRefusal::Domain);
    }
    Ok(())
}
