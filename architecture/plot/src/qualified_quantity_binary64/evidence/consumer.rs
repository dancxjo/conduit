//! One bounded entrance for Host producers and independently prepared consumers.
//! The caller supplies retained artifacts, the exact boundary basis and the
//! original admitted numerical allowance; observation is not execution attestation.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualifiedQuantityBinary64EvidenceRefusal {
    Computation(QualifiedQuantityBinary64Refusal),
    Report(ProjectionRefusal),
    Transport(ProjectionEvidenceRefusal),
}
use QualifiedQuantityBinary64EvidenceRefusal as E;
impl From<ProjectionRefusal> for E {
    fn from(value: ProjectionRefusal) -> Self {
        Self::Report(value)
    }
}
impl From<ProjectionEvidenceRefusal> for E {
    fn from(value: ProjectionEvidenceRefusal) -> Self {
        Self::Transport(value)
    }
}
impl QualifiedQuantityBinary64Result<'_> {
    /// Writes shared CPE into caller-owned staging storage. Exact artifact
    /// references require retaining canonical_program() and incoming_evidence().
    /// Commit only the returned span after successful encoding.
    pub fn encode_evidence(
        &self,
        basis: ProjectionBasis<'_>,
        output: &mut [u8],
    ) -> Result<usize, E> {
        let domain = QualifiedQuantityBinary64Domain::new();
        let policy = self.policy();
        let codec = QualifiedQuantityBinary64EvidenceCodec::new(policy);
        let target = self.target();
        let facts = self.projection_facts();
        let report =
            ProjectionReport::new(&domain, &policy, report_input(self, &target, &facts, basis))?;
        report.require_bound_policy(policy.identity(), policy.configuration_digest().unwrap())?;
        Ok(encode_projection_evidence(
            &report,
            &codec,
            QUALIFIED_QUANTITY_BINARY64_EVIDENCE_LIMITS,
            output,
        )?)
    }
}
impl PreparedQualifiedQuantityBinary64 {
    /// Replays incoming evidence and this independently checked program before
    /// comparing every canonical CPE byte and requiring this exact policy.
    /// A changed basis, original allowance, artifact or selected profile refuses.
    /// The finite transport ceiling is checked before numerical replay. Failed
    /// numerical replay retains the runner's transactional failure behavior.
    pub fn verify_evidence(
        &mut self,
        incoming: &[u8],
        basis: ProjectionBasis<'_>,
        evidence: &[u8],
        work: &mut ExactNumericWork,
    ) -> Result<QualifiedQuantityBinary64Target, E> {
        if evidence.len() > QUALIFIED_QUANTITY_BINARY64_EVIDENCE_LIMITS.total_bytes {
            return Err(E::Transport(ProjectionEvidenceRefusal::Extent));
        }
        let policy = self.policy();
        let result = self.execute(incoming, work).map_err(E::Computation)?;
        let domain = QualifiedQuantityBinary64Domain::new();
        let codec = QualifiedQuantityBinary64EvidenceCodec::new(policy);
        let target = result.target();
        let facts = result.projection_facts();
        let report = ProjectionReport::new(
            &domain,
            &policy,
            report_input(&result, &target, &facts, basis),
        )?;
        verify_projection_evidence(
            &report,
            &codec,
            QUALIFIED_QUANTITY_BINARY64_EVIDENCE_LIMITS,
            evidence,
        )?;
        report.require_bound_policy(policy.identity(), policy.configuration_digest().unwrap())?;
        Ok(target)
    }
}
fn report_input<'a, 's>(
    source: &'a QualifiedQuantityBinary64Result<'s>,
    target: &'a QualifiedQuantityBinary64Target,
    facts: &'a [ProjectionFact<'a, QualifiedQuantityBinary64Domain<'s>>],
    basis: ProjectionBasis<'a>,
) -> ProjectionInput<'a, QualifiedQuantityBinary64Domain<'s>> {
    ProjectionInput {
        basis,
        source,
        target: Some(target),
        facts,
        native: &[],
        scores: &[],
        admitted: &[],
        attempts: &[],
        selected_attempt: None,
        mechanism: ProjectionMechanism::Completed,
        diagnostics: &[],
    }
}
