//! Semantic replay against independently retained input/output and selected policy.
//! The caller supplies the admitted report basis and original producer allowance;
//! neither is adopted from untrusted evidence. Replay does not attest execution.
use super::*;
use crate::projection::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantityCoordinateReplayRefusal {
    Work(ExactNumericRefusal),
    Conversion(QuantityCoordinateConversionRefusal),
    Target,
    Report(ProjectionRefusal),
    Evidence(ProjectionEvidenceRefusal),
}
use QuantityCoordinateReplayRefusal as R;

impl PreparedQuantityCoordinateConversion {
    /// Scratch must hold a complete converted Quantity. It is staging storage,
    /// not a committed output. All storage is supplied/prepared before Play.
    /// The consuming work meter covers producer replay and evidence passes;
    /// the producer allowance remains a separate, exact report input.
    #[allow(clippy::too_many_arguments)]
    pub fn verify_evidence(
        &mut self,
        source: &[u8],
        target: &[u8],
        policy: QuantityCoordinateConversionPolicy,
        producer_allowance: u64,
        basis: ProjectionBasis<'_>,
        evidence: &[u8],
        scratch: &mut [u8],
        work: &mut ExactNumericWork,
    ) -> Result<(), R> {
        let limits = QUANTITY_COORDINATE_CONVERSION_EVIDENCE_LIMITS;
        if evidence.len() > limits.total_bytes {
            return Err(R::Evidence(ProjectionEvidenceRefusal::Extent));
        }
        // Reserve the exact producer envelope up front. Failed replay is charged
        // too; no retry with a substituted producer allowance is performed.
        if work.remaining() < producer_allowance {
            return Err(R::Work(ExactNumericRefusal::WorkLimit));
        }
        let mut replay = ExactNumericWork::new(producer_allowance);
        let result = self.execute(source, policy.target_unit, scratch, &mut replay);
        work.charge(replay.spent()).map_err(R::Work)?;
        let receipt = result.map_err(R::Conversion)?;
        let converted = &scratch[..receipt.encoded_bytes()];
        work.charge(target.len() as u64).map_err(R::Work)?;
        if converted != target {
            return Err(R::Target);
        }
        let target = QuantityValueProfile::Mathematical
            .decode(crate::QUANTITY_INFO_ID, target, work)
            .map_err(|_| R::Target)?;
        // Codec measurement and canonical comparison each traverse complete
        // bounded values, including when the supplied evidence is too short.
        // Charge the admitted envelope rather than attacker-chosen byte length.
        work.charge(8 * limits.total_bytes as u64)
            .map_err(R::Work)?;
        let domain = QuantityCoordinateConversionDomain::new();
        let facts = domain.facts(&receipt);
        let report = ProjectionReport::new(
            &domain,
            &policy,
            ProjectionInput {
                basis,
                source: &receipt,
                target: Some(&target),
                facts: &facts,
                native: &[],
                scores: &[],
                admitted: &[],
                attempts: &[],
                selected_attempt: None,
                mechanism: ProjectionMechanism::Completed,
                diagnostics: &[],
            },
        )
        .map_err(R::Report)?;
        report
            .require_bound_policy(policy.identity(), policy.configuration_digest().unwrap())
            .map_err(R::Report)?;
        verify_projection_evidence(
            &report,
            &QuantityCoordinateConversionEvidenceCodec::new(policy),
            limits,
            evidence,
        )
        .map_err(R::Evidence)
    }
}
