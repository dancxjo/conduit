//! Bounded interpretations of evidence. These objects never grant authority.
//!
//! Preparation admits borrowed, immutable domain data; operations allocate nothing.
//! Domain validation must check the complete target/value and their finite bounds.
use crate::SignIdentity;

pub const MAX_CLAIM_EDGES: usize = 16;
pub const MAX_CLAIM_CANDIDATES: usize = 32;
pub const MAX_CLAIM_TEXT_BYTES: usize = 192;
pub const MAX_CLAIM_CHANGES: usize = 4;

/// Exact identity or short inspectable wording (at most 192 UTF-8 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ClaimText<'a>(&'a str);
impl<'a> ClaimText<'a> {
    pub fn new(text: &'a str) -> Result<Self, ClaimRefusal> {
        if text.is_empty() || text.len() > MAX_CLAIM_TEXT_BYTES {
            return Err(ClaimRefusal::TextBound);
        }
        Ok(Self(text))
    }
    pub fn as_str(self) -> &'a str {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClaimReason<'a> {
    pub profile: ClaimText<'a>,
    pub explanation: ClaimText<'a>,
}

/// Domains own exact occurrence/generation targets and typed assertion contracts.
/// This validator must reject malformed or unbounded domain data. Implementations
/// are trusted semantic contracts, not hostile-code confinement mechanisms.
pub trait ClaimDomain {
    type Target: Eq;
    type Value;
    fn contract(&self) -> ClaimText<'_>;
    fn validate(&self, target: &Self::Target, value: &Self::Value) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimSupport<'a> {
    Sign(&'a SignIdentity),
    Claim(ClaimText<'a>),
    SourceGeneration {
        source: ClaimText<'a>,
        generation: ClaimText<'a>,
    },
}

/// Scores have no ordering implementation. Comparison explicitly checks the
/// entire scale/calibration/producer contract first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClaimScore<'a> {
    scale: ClaimText<'a>,
    calibration: Option<ClaimText<'a>>,
    producer: ClaimText<'a>,
    minimum: i64,
    maximum: i64,
    value: i64,
}
impl<'a> ClaimScore<'a> {
    pub fn new(
        scale: ClaimText<'a>,
        calibration: Option<ClaimText<'a>>,
        producer: ClaimText<'a>,
        minimum: i64,
        maximum: i64,
        value: i64,
    ) -> Result<Self, ClaimRefusal> {
        if minimum >= maximum || value < minimum || value > maximum {
            return Err(ClaimRefusal::ScoreBound);
        }
        Ok(Self {
            scale,
            calibration,
            producer,
            minimum,
            maximum,
            value,
        })
    }
    pub fn compare(&self, other: &Self) -> Result<core::cmp::Ordering, ClaimRefusal> {
        if (
            self.scale,
            self.calibration,
            self.producer,
            self.minimum,
            self.maximum,
        ) != (
            other.scale,
            other.calibration,
            other.producer,
            other.minimum,
            other.maximum,
        ) {
            return Err(ClaimRefusal::UnrelatedScores);
        }
        Ok(self.value.cmp(&other.value))
    }
    pub fn value(&self) -> i64 {
        self.value
    }
    pub fn scale(&self) -> ClaimText<'a> {
        self.scale
    }
    pub fn calibration(&self) -> Option<ClaimText<'a>> {
        self.calibration
    }
    pub fn producer(&self) -> ClaimText<'a> {
        self.producer
    }
    pub fn bounds(&self) -> (i64, i64) {
        (self.minimum, self.maximum)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimLifecycle {
    Proposed,
    Stable,
    Committed,
    Revised,
    Invalidated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClaimChange<'a> {
    pub state: ClaimLifecycle,
    pub reason: ClaimReason<'a>,
    pub replacement: Option<ClaimText<'a>>,
}

/// Supplied during preparation. Evidence references retain exact identities;
/// they neither prove the referenced evidence exists nor authorize its use.
#[derive(Debug, Clone, Copy)]
pub struct ClaimBasis<'a> {
    pub identity: ClaimText<'a>,
    pub producer: ClaimText<'a>,
    pub artifact: ClaimText<'a>,
    pub support: &'a [ClaimSupport<'a>],
    pub conflicts: &'a [ClaimText<'a>],
    pub score: Option<ClaimScore<'a>>,
    pub rationale: ClaimReason<'a>,
}

pub struct SemanticClaim<'a, D: ClaimDomain> {
    domain: &'a D,
    target: &'a D::Target,
    value: &'a D::Value,
    basis: ClaimBasis<'a>,
    history: [Option<ClaimChange<'a>>; MAX_CLAIM_CHANGES],
    changes: usize,
}
impl<'a, D: ClaimDomain> SemanticClaim<'a, D> {
    pub fn new(
        domain: &'a D,
        target: &'a D::Target,
        value: &'a D::Value,
        basis: ClaimBasis<'a>,
    ) -> Result<Self, ClaimRefusal> {
        if !domain.validate(target, value) {
            return Err(ClaimRefusal::Domain);
        }
        if basis.support.is_empty()
            || basis.support.len() > MAX_CLAIM_EDGES
            || basis.conflicts.len() > MAX_CLAIM_EDGES
        {
            return Err(ClaimRefusal::EdgeBound);
        }
        for (index, edge) in basis.support.iter().enumerate() {
            if basis.support[..index].contains(edge)
                || matches!(edge, ClaimSupport::Claim(id) if *id == basis.identity)
            {
                return Err(ClaimRefusal::InvalidEdge);
            }
            if let ClaimSupport::Sign(sign) = edge {
                for text in [
                    sign.sign_id.as_str(),
                    sign.host_id.as_str(),
                    sign.boot_id.as_str(),
                ] {
                    ClaimText::new(text)?;
                }
                if let Some(play) = &sign.active_play_id {
                    ClaimText::new(play.as_str())?;
                }
            }
        }
        for (index, id) in basis.conflicts.iter().enumerate() {
            if *id == basis.identity
                || basis.conflicts[..index].contains(id)
                || basis.support.contains(&ClaimSupport::Claim(*id))
            {
                return Err(ClaimRefusal::InvalidEdge);
            }
        }
        if basis
            .score
            .is_some_and(|score| score.producer != basis.producer)
        {
            return Err(ClaimRefusal::ScoreProducer);
        }
        Ok(Self {
            domain,
            target,
            value,
            basis,
            history: [None; MAX_CLAIM_CHANGES],
            changes: 0,
        })
    }
    pub fn basis(&self) -> &ClaimBasis<'a> {
        &self.basis
    }
    pub fn target(&self) -> &D::Target {
        self.target
    }
    pub fn value(&self) -> &D::Value {
        self.value
    }
    pub fn contract(&self) -> ClaimText<'_> {
        self.domain.contract()
    }
    pub fn history(&self) -> &[Option<ClaimChange<'a>>] {
        &self.history[..self.changes]
    }
    pub fn lifecycle(&self) -> ClaimLifecycle {
        self.history[..self.changes]
            .last()
            .and_then(|entry| *entry)
            .map_or(ClaimLifecycle::Proposed, |change| change.state)
    }
    /// Only lifecycle changes. Identity, value, target and provenance are locked.
    /// Committed corrections require a new supported claim, never mutation.
    pub fn transition(&mut self, change: ClaimChange<'a>) -> Result<(), ClaimRefusal> {
        use ClaimLifecycle::*;
        let allowed = matches!(
            (self.lifecycle(), change.state),
            (Proposed, Stable | Revised | Invalidated)
                | (Stable, Committed | Revised | Invalidated)
        );
        if !allowed {
            return Err(ClaimRefusal::Lifecycle);
        }
        if (change.state == Revised) != change.replacement.is_some()
            || change.replacement == Some(self.basis.identity)
        {
            return Err(ClaimRefusal::Replacement);
        }
        self.history[self.changes] = Some(change);
        self.changes += 1;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimRefusal {
    TextBound,
    Domain,
    EdgeBound,
    InvalidEdge,
    ScoreBound,
    UnrelatedScores,
    ScoreProducer,
    Lifecycle,
    Replacement,
    CandidateBound,
    DuplicateIdentity,
    TargetMismatch,
    ContractMismatch,
    CyclicSupport,
    MissingSupport,
    InvalidDecision,
    NonCanonicalOrder,
}

/// A policy is a reviewed pure function of this complete, canonically ordered
/// candidate set. Its exact identity includes configuration and version. Core
/// does not choose a universal ordering or source-priority ladder.
pub trait ClaimPolicy<D: ClaimDomain> {
    fn identity(&self) -> ClaimText<'_>;
    fn assess<'a>(
        &self,
        claim: &SemanticClaim<'a, D>,
        evidence: &[&SemanticClaim<'a, D>],
    ) -> Option<ClaimReason<'a>>;
    fn decide<'a>(
        &self,
        candidates: &[&SemanticClaim<'a, D>],
        eligible: &[bool],
        evidence: &[&SemanticClaim<'a, D>],
    ) -> ClaimDecision<'a>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimDecision<'a> {
    Selected {
        identity: ClaimText<'a>,
        reason: ClaimReason<'a>,
    },
    Abstained {
        reason: ClaimReason<'a>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClaimCandidateReceipt<'a> {
    pub identity: ClaimText<'a>,
    pub lifecycle: ClaimLifecycle,
    pub changes: usize,
    pub exclusion: Option<ClaimReason<'a>>,
}

pub struct ClaimResolution<'a, D: ClaimDomain> {
    identity: ClaimText<'a>,
    policy: ClaimText<'a>,
    candidates: &'a [&'a SemanticClaim<'a, D>],
    evidence: &'a [&'a SemanticClaim<'a, D>],
    receipts: [Option<ClaimCandidateReceipt<'a>>; MAX_CLAIM_CANDIDATES],
    decision: ClaimDecision<'a>,
}

impl<'a, D: ClaimDomain> ClaimResolution<'a, D> {
    pub fn identity(&self) -> ClaimText<'a> {
        self.identity
    }
    pub fn policy(&self) -> ClaimText<'a> {
        self.policy
    }
    pub fn candidates(&self) -> &[&SemanticClaim<'a, D>] {
        self.candidates
    }
    pub fn evidence(&self) -> &[&SemanticClaim<'a, D>] {
        self.evidence
    }
    pub fn receipts(&self) -> &[Option<ClaimCandidateReceipt<'a>>] {
        &self.receipts[..self.candidates.len()]
    }
    pub fn decision(&self) -> ClaimDecision<'a> {
        self.decision
    }
}

/// All claim-to-claim support must resolve within this bounded evidence closure.
/// Conflict may be cyclic (disagreement is symmetric); support must be acyclic.
/// Callers retain the immutable claims with the receipt to inspect exact truth.
pub fn resolve_claims<'a, D: ClaimDomain>(
    identity: ClaimText<'a>,
    candidates: &'a [&'a SemanticClaim<'a, D>],
    policy: &'a impl ClaimPolicy<D>,
) -> Result<ClaimResolution<'a, D>, ClaimRefusal> {
    if candidates.is_empty() || candidates.len() > MAX_CLAIM_CANDIDATES {
        return Err(ClaimRefusal::CandidateBound);
    }
    resolve_claims_with_evidence(identity, candidates, candidates, policy)
}

/// A separate finite support closure permits claims about other exact targets
/// to support candidates. Every candidate must be the same immutable object as
/// its closure entry, preventing an ID from substituting different evidence.
pub fn resolve_claims_with_evidence<'a, D: ClaimDomain>(
    identity: ClaimText<'a>,
    candidates: &'a [&'a SemanticClaim<'a, D>],
    evidence: &'a [&'a SemanticClaim<'a, D>],
    policy: &'a impl ClaimPolicy<D>,
) -> Result<ClaimResolution<'a, D>, ClaimRefusal> {
    if candidates.is_empty()
        || candidates.len() > MAX_CLAIM_CANDIDATES
        || evidence.is_empty()
        || evidence.len() > MAX_CLAIM_CANDIDATES
    {
        return Err(ClaimRefusal::CandidateBound);
    }
    for (i, claim) in candidates.iter().enumerate() {
        if claim.target() != candidates[0].target() {
            return Err(ClaimRefusal::TargetMismatch);
        }
        if claim.contract() != candidates[0].contract() {
            return Err(ClaimRefusal::ContractMismatch);
        }
        if !evidence.iter().any(|other| core::ptr::eq(*claim, *other)) {
            return Err(ClaimRefusal::MissingSupport);
        }
        if i > 0 {
            check_identity_order(candidates[i - 1].basis.identity, claim.basis.identity)?;
        }
    }
    let mut reach = [[false; MAX_CLAIM_CANDIDATES]; MAX_CLAIM_CANDIDATES];
    for (i, claim) in evidence.iter().enumerate() {
        if i > 0 {
            check_identity_order(evidence[i - 1].basis.identity, claim.basis.identity)?;
        }
        for edge in claim.basis.support {
            if let ClaimSupport::Claim(id) = edge {
                let j = evidence
                    .iter()
                    .position(|other| other.basis.identity == *id)
                    .ok_or(ClaimRefusal::MissingSupport)?;
                reach[i][j] = true;
            }
        }
    }
    for k in 0..evidence.len() {
        for i in 0..evidence.len() {
            for j in 0..evidence.len() {
                reach[i][j] |= reach[i][k] && reach[k][j];
            }
        }
    }
    if (0..evidence.len()).any(|i| reach[i][i]) {
        return Err(ClaimRefusal::CyclicSupport);
    }
    let mut receipts = [None; MAX_CLAIM_CANDIDATES];
    let mut eligible = [false; MAX_CLAIM_CANDIDATES];
    for (i, claim) in candidates.iter().enumerate() {
        let exclusion = if matches!(
            claim.lifecycle(),
            ClaimLifecycle::Revised | ClaimLifecycle::Invalidated
        ) {
            Some(ClaimReason {
                profile: ClaimText("claim/retired@1"),
                explanation: ClaimText("Retained history is not eligible for selection"),
            })
        } else {
            policy.assess(claim, evidence)
        };
        eligible[i] = exclusion.is_none();
        receipts[i] = Some(ClaimCandidateReceipt {
            identity: claim.basis.identity,
            lifecycle: claim.lifecycle(),
            changes: claim.changes,
            exclusion,
        });
    }
    let decision = policy.decide(candidates, &eligible[..candidates.len()], evidence);
    if let ClaimDecision::Selected { identity, .. } = decision {
        if !candidates
            .iter()
            .enumerate()
            .any(|(i, c)| c.basis.identity == identity && eligible[i])
        {
            return Err(ClaimRefusal::InvalidDecision);
        }
    }
    Ok(ClaimResolution {
        identity,
        policy: policy.identity(),
        candidates,
        evidence,
        receipts,
        decision,
    })
}

fn check_identity_order(
    previous: ClaimText<'_>,
    current: ClaimText<'_>,
) -> Result<(), ClaimRefusal> {
    match previous.cmp(&current) {
        core::cmp::Ordering::Equal => Err(ClaimRefusal::DuplicateIdentity),
        core::cmp::Ordering::Greater => Err(ClaimRefusal::NonCanonicalOrder),
        _ => Ok(()),
    }
}
