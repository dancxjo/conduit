//! Historical hard-admission account when the cost policy itself refuses.
use super::{cost_explanation::BoundedText, CostExplanationRefusal};
use alloc::string::String;
use conduit_planner::{
    resource_cost::*, PerformanceIntent, RealizationDecisionDisposition,
    MAXIMUM_REALIZATION_DECISION_RECORDS,
};
use core::fmt::{self, Write};

pub(super) fn render(refusal: &CostPolicyRefusal) -> Result<String, CostExplanationRefusal> {
    let records = &refusal.realization.signs;
    if records.is_empty() {
        return Err(CostExplanationRefusal::NoRetainedDecision);
    }
    if records.len() > MAXIMUM_REALIZATION_DECISION_RECORDS {
        return Err(CostExplanationRefusal::Extent);
    }
    let admitted = records
        .iter()
        .filter(|record| {
            !matches!(
                record.disposition,
                RealizationDecisionDisposition::Rejected(_)
            )
        })
        .count();
    let valid_reason = match refusal.reason {
        CostEvidenceRefusal::InputLimit => admitted > MAXIMUM_COST_CANDIDATES,
        CostEvidenceRefusal::UnsupportedIntent => {
            admitted <= MAXIMUM_COST_CANDIDATES
                && refusal.intent == PerformanceIntent::BoundedResponse
        }
        CostEvidenceRefusal::AmbiguousEvidence => {
            admitted <= MAXIMUM_COST_CANDIDATES
                && refusal.intent != PerformanceIntent::BoundedResponse
        }
        _ => false,
    };
    if !valid_reason
        || admitted == 0
        || records
            .iter()
            .filter(|record| record.disposition == RealizationDecisionDisposition::Selected)
            .count()
            != 1
        || !records.iter().any(|record| {
            record.host_id == refusal.realization.choice.host_id
                && record.capability_id == refusal.realization.choice.capability_id
                && record.disposition == RealizationDecisionDisposition::Selected
        })
    {
        return Err(CostExplanationRefusal::InconsistentSelection);
    }
    for (index, record) in records.iter().enumerate() {
        if records[..index].iter().any(|other| {
            other.host_id == record.host_id && other.capability_id == record.capability_id
        }) {
            return Err(CostExplanationRefusal::InconsistentSelection);
        }
    }
    let mut out = BoundedText(String::new());
    let render = |out: &mut BoundedText| -> fmt::Result {
        writeln!(out,"historical planner cost policy refused; no cost-admitted winner; reason={:?} intent={:?} admitted_candidates={} maximum_cost_candidates={}",
            refusal.reason,refusal.intent,admitted,MAXIMUM_COST_CANDIDATES)?;
        for record in records {
            let disposition = if record.disposition == RealizationDecisionDisposition::Selected {
                &RealizationDecisionDisposition::Admitted
            } else {
                &record.disposition
            };
            writeln!(out,"candidate host={} boot={} generation={} capability={} implementation={} artifact={} hard_admission={disposition:?} hard_clause_source={:?}",
                record.host_id.as_str().escape_default(),record.boot_id.as_str().escape_default(),
                record.offer_generation.0,record.capability_id.as_str().escape_default(),
                record.implementation_id.as_str().escape_default(),record.artifact_id.as_str().escape_default(),record.clause_source)?;
        }
        Ok(())
    };
    render(&mut out).map_err(|_| CostExplanationRefusal::Extent)?;
    Ok(out.0)
}
