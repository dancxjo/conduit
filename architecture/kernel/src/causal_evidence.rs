//! Bounded exact causal evidence beside, never instead of, runtime Signs.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CausalRelationship {
    DerivedFrom,
    CausedBy,
    RequestedBy,
    AdmittedBy,
    RefusedBy,
    RealizedBy,
    ObservedFrom,
    TerminatedBecause,
    Supersedes,
    Corrects,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct EvidenceIdentity {
    pub sign: u64,
    pub execution: u64,
    pub host_session: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CausalEdge {
    pub effect: EvidenceIdentity,
    pub relationship: CausalRelationship,
    pub cause: EvidenceIdentity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CausalEvidenceRefusal {
    AmbiguousCorrelation,
    CapacityExhausted,
    Duplicate,
    SelfCausation,
    Cycle,
    TooManyDirectPredecessors,
    StaleSession,
    TruncatedHistory,
    Unknown,
}

/// One effect may branch to several material causes, but the explanatory fan-in
/// remains finite independently of the store's total retention capacity.
pub const MAXIMUM_DIRECT_CAUSAL_PREDECESSORS: usize = 8;

/// Exact bounded bridge from the digest carried by a semantic terminal value
/// to the retained evidence identity that begins its causal explanation.
///
/// Several terminal observations may truthfully carry the same digest. The
/// index retains each observation rather than silently choosing one; callers
/// must supply a narrower evidence envelope or report ambiguity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalEvidenceCorrelation {
    pub cause_digest: [u8; 32],
    pub terminal: EvidenceIdentity,
}

pub struct TerminalEvidenceIndex<const N: usize> {
    correlations: [Option<TerminalEvidenceCorrelation>; N],
    len: usize,
    history_truncated: bool,
}

impl<const N: usize> Default for TerminalEvidenceIndex<N> {
    fn default() -> Self {
        Self {
            correlations: [None; N],
            len: 0,
            history_truncated: false,
        }
    }
}

impl<const N: usize> TerminalEvidenceIndex<N> {
    pub fn record(
        &mut self,
        correlation: TerminalEvidenceCorrelation,
    ) -> Result<(), CausalEvidenceRefusal> {
        if self.correlations[..self.len]
            .iter()
            .flatten()
            .any(|existing| existing == &correlation)
        {
            return Err(CausalEvidenceRefusal::Duplicate);
        }
        if N == 0 {
            return Err(CausalEvidenceRefusal::CapacityExhausted);
        }
        if self.len == N {
            self.correlations.copy_within(1..N, 0);
            self.len -= 1;
            self.history_truncated = true;
        }
        self.correlations[self.len] = Some(correlation);
        self.len += 1;
        Ok(())
    }

    /// Resolves a semantic terminal's cause digest without guessing. An empty
    /// result after compaction is distinguishable from a digest that was never
    /// present, because aged evidence cannot truthfully be called unknown.
    pub fn terminal_for(
        &self,
        cause_digest: [u8; 32],
    ) -> Result<EvidenceIdentity, CausalEvidenceRefusal> {
        let mut matches = self.correlations[..self.len]
            .iter()
            .flatten()
            .filter(|correlation| correlation.cause_digest == cause_digest);
        let first = matches.next().map(|correlation| correlation.terminal);
        if matches.next().is_some() {
            return Err(CausalEvidenceRefusal::AmbiguousCorrelation);
        }
        first.ok_or(if self.history_truncated {
            CausalEvidenceRefusal::TruncatedHistory
        } else {
            CausalEvidenceRefusal::Unknown
        })
    }

    pub const fn history_was_truncated(&self) -> bool {
        self.history_truncated
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CausalTraceCompleteness {
    Complete,
    /// At least one older edge has aged out of this evidence envelope. The
    /// retained graph remains truthful, but it cannot establish a root cause.
    IncompleteHistory,
}

pub struct CausalTrace<const N: usize> {
    terminal: EvidenceIdentity,
    edges: [Option<CausalEdge>; N],
    len: usize,
    completeness: CausalTraceCompleteness,
}

impl<const N: usize> CausalTrace<N> {
    pub const fn terminal(&self) -> EvidenceIdentity {
        self.terminal
    }

    pub fn edges(&self) -> impl Iterator<Item = &CausalEdge> {
        self.edges[..self.len].iter().flatten()
    }

    pub const fn completeness(&self) -> CausalTraceCompleteness {
        self.completeness
    }
}

pub struct CausalEvidence<const N: usize> {
    edges: [Option<CausalEdge>; N],
    len: usize,
    history_truncated: bool,
}

impl<const N: usize> Default for CausalEvidence<N> {
    fn default() -> Self {
        Self {
            edges: [None; N],
            len: 0,
            history_truncated: false,
        }
    }
}

impl<const N: usize> CausalEvidence<N> {
    pub fn record(&mut self, edge: CausalEdge) -> Result<(), CausalEvidenceRefusal> {
        if edge.effect == edge.cause {
            return Err(CausalEvidenceRefusal::SelfCausation);
        }
        if edge.effect.execution != edge.cause.execution
            && edge.effect.host_session == edge.cause.host_session
        {
            return Err(CausalEvidenceRefusal::StaleSession);
        }
        if self.edges[..self.len]
            .iter()
            .flatten()
            .any(|existing| existing == &edge)
        {
            return Err(CausalEvidenceRefusal::Duplicate);
        }
        if self.edges[..self.len]
            .iter()
            .flatten()
            .filter(|existing| existing.effect == edge.effect)
            .count()
            >= MAXIMUM_DIRECT_CAUSAL_PREDECESSORS
        {
            return Err(CausalEvidenceRefusal::TooManyDirectPredecessors);
        }
        if self.would_create_cycle(edge.effect, edge.cause) {
            return Err(CausalEvidenceRefusal::Cycle);
        }
        if N == 0 {
            return Err(CausalEvidenceRefusal::CapacityExhausted);
        }
        if self.len == N {
            self.edges.copy_within(1..N, 0);
            self.len -= 1;
            self.history_truncated = true;
        }
        self.edges[self.len] = Some(edge);
        self.len += 1;
        Ok(())
    }

    pub const fn history_was_truncated(&self) -> bool {
        self.history_truncated
    }

    /// Reconstructs only explicit predecessor edges reachable from `terminal`.
    /// Array order is deterministic retention order; no clock adjacency is used.
    pub fn trace(
        &self,
        terminal: EvidenceIdentity,
    ) -> Result<CausalTrace<N>, CausalEvidenceRefusal> {
        if !self.edges[..self.len]
            .iter()
            .flatten()
            .any(|edge| edge.effect == terminal || edge.cause == terminal)
        {
            return Err(CausalEvidenceRefusal::Unknown);
        }

        let mut trace = CausalTrace {
            terminal,
            edges: [None; N],
            len: 0,
            completeness: if self.history_truncated {
                CausalTraceCompleteness::IncompleteHistory
            } else {
                CausalTraceCompleteness::Complete
            },
        };
        let mut frontier = [None; N];
        let mut frontier_len = 0;
        if N > 0 {
            frontier[frontier_len] = Some(terminal);
            frontier_len += 1;
        }
        let mut cursor = 0;
        while cursor < frontier_len {
            let effect = frontier[cursor].expect("frontier active prefix is populated");
            cursor += 1;
            for edge in self.edges[..self.len]
                .iter()
                .flatten()
                .filter(|edge| edge.effect == effect)
            {
                if trace.edges[..trace.len]
                    .iter()
                    .flatten()
                    .any(|retained| retained == edge)
                {
                    continue;
                }
                trace.edges[trace.len] = Some(*edge);
                trace.len += 1;
                if edge.cause != terminal
                    && !frontier[..frontier_len]
                        .iter()
                        .flatten()
                        .any(|identity| identity == &edge.cause)
                    && frontier_len < N
                {
                    frontier[frontier_len] = Some(edge.cause);
                    frontier_len += 1;
                }
            }
        }
        Ok(trace)
    }

    fn would_create_cycle(&self, effect: EvidenceIdentity, cause: EvidenceIdentity) -> bool {
        if N == 0 {
            return false;
        }
        let mut frontier = [None; N];
        frontier[0] = Some(cause);
        let mut frontier_len = 1;
        let mut cursor = 0;
        while cursor < frontier_len {
            let current = frontier[cursor].expect("frontier active prefix is populated");
            cursor += 1;
            for edge in self.edges[..self.len]
                .iter()
                .flatten()
                .filter(|edge| edge.effect == current)
            {
                if edge.cause == effect {
                    return true;
                }
                if !frontier[..frontier_len]
                    .iter()
                    .flatten()
                    .any(|identity| identity == &edge.cause)
                    && frontier_len < N
                {
                    frontier[frontier_len] = Some(edge.cause);
                    frontier_len += 1;
                }
            }
        }
        false
    }

    pub fn cause_of(
        &self,
        effect: EvidenceIdentity,
        relationship: CausalRelationship,
    ) -> Result<EvidenceIdentity, CausalEvidenceRefusal> {
        let mut matches = self.edges[..self.len]
            .iter()
            .flatten()
            .filter(|edge| edge.effect == effect && edge.relationship == relationship);
        let first = matches.next().ok_or(CausalEvidenceRefusal::Unknown)?;
        if matches.next().is_some() {
            return Err(CausalEvidenceRefusal::Unknown);
        }
        Ok(first.cause)
    }
}
