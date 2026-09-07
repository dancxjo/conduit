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
    CapacityExhausted,
    Duplicate,
    SelfCausation,
    StaleSession,
    Unknown,
}

pub struct CausalEvidence<const N: usize> {
    edges: [Option<CausalEdge>; N],
    len: usize,
}

impl<const N: usize> Default for CausalEvidence<N> {
    fn default() -> Self {
        Self {
            edges: [None; N],
            len: 0,
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
        if self.len == N {
            return Err(CausalEvidenceRefusal::CapacityExhausted);
        }
        self.edges[self.len] = Some(edge);
        self.len += 1;
        Ok(())
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
