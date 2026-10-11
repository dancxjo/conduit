//! Historical projection of a refused exact comparison, with no reranking.
use super::{
    cost_explanation::{exact, BoundedText},
    CostExplanationRefusal,
};
use alloc::string::String;
use conduit_planner::{resource_cost::*, RealizationDecisionDisposition};
use core::fmt::{self, Write};

pub(super) fn render(refusal: &CostComparisonRefusal) -> Result<String, CostExplanationRefusal> {
    let records = &refusal.realization.signs;
    if records.is_empty() {
        return Err(CostExplanationRefusal::NoRetainedDecision);
    }
    if records.len() > conduit_planner::MAXIMUM_REALIZATION_DECISION_RECORDS {
        return Err(CostExplanationRefusal::Extent);
    }
    if !matches!(refusal.reason, CostEvidenceRefusal::Numeric(_))
        || refusal
            .arithmetic_work_spent
            .checked_add(refusal.arithmetic_work_remaining)
            != Some(refusal.arithmetic_work_limit)
        || (refusal.operands[0].source.host_id == refusal.operands[1].source.host_id
            && refusal.operands[0].source.capability_id == refusal.operands[1].source.capability_id)
    {
        return Err(CostExplanationRefusal::InconsistentSelection);
    }
    if records
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
    for operand in &refusal.operands {
        if operand.upper_seconds.is_negative()
            || operand
                .source
                .window
                .validate_at(&operand.evaluated_at)
                .is_err()
            || operand.expected_condition.is_some_and(|expected| {
                operand
                    .current_execution
                    .as_ref()
                    .is_none_or(|(_, observed)| *observed != expected)
            })
        {
            return Err(CostExplanationRefusal::InconsistentSelection);
        }
        for (source, needs_fresh) in [
            (Some(&operand.source), true),
            (operand.calibration.as_ref(), true),
            (
                operand.current_execution.as_ref().map(|(source, _)| source),
                true,
            ),
            (operand.execution_assumption.as_ref(), false),
        ] {
            let Some(source) = source else {
                continue;
            };
            if !records.iter().any(|record| {
                record.host_id == source.host_id
                    && record.boot_id == source.boot_id
                    && record.offer_generation == source.offer_generation
                    && record.capability_id == source.capability_id
                    && record.implementation_id == source.implementation_id
                    && record.artifact_id == source.implementation_artifact
                    && !matches!(
                        record.disposition,
                        RealizationDecisionDisposition::Rejected(_)
                    )
            }) || source.host_id != operand.source.host_id
                || source.capability_id != operand.source.capability_id
                || source.execution_profile_id != operand.source.execution_profile_id
                || source.host_profile != operand.source.host_profile
                || source.kind_id != operand.source.kind_id
                || source.kind_contract_revision != operand.source.kind_contract_revision
                || source.calibration_identity != operand.source.calibration_identity
                || source.calibration_revision != operand.source.calibration_revision
                || source.resource != operand.source.resource
                || (needs_fresh && source.window.validate_at(&operand.evaluated_at).is_err())
            {
                return Err(CostExplanationRefusal::InconsistentSelection);
            }
        }
    }
    let mut out = BoundedText(String::new());
    let render = |out: &mut BoundedText| -> fmt::Result {
        writeln!(out,"historical planner cost comparison refused; no cost-admitted winner; reason={:?} intent={:?} arithmetic_work_limit={} spent={} remaining={}",
            refusal.reason,refusal.intent,refusal.arithmetic_work_limit,refusal.arithmetic_work_spent,refusal.arithmetic_work_remaining)?;
        for record in records {
            let disposition = if record.disposition == RealizationDecisionDisposition::Selected {
                &RealizationDecisionDisposition::Admitted
            } else {
                &record.disposition
            };
            writeln!(out,"candidate host={} boot={} generation={} capability={} hard_admission={disposition:?} hard_clause_source={:?}",
                record.host_id.as_str().escape_default(),record.boot_id.as_str().escape_default(),record.offer_generation.0,
                record.capability_id.as_str().escape_default(),record.clause_source)?;
        }
        for (index, operand) in refusal.operands.iter().enumerate() {
            write!(out, "compared operand={index} upper_fulfillment_seconds=")?;
            exact(operand.upper_seconds, out)?;
            writeln!(
                out,
                " source={:?} calibration={:?} evaluated_at={:?}",
                operand.source, operand.calibration, operand.evaluated_at
            )?;
            writeln!(out,"current_execution={:?} expected_condition={:?} execution_assumption={:?} (historical attribution)",
                operand.current_execution,operand.expected_condition,operand.execution_assumption)?;
        }
        Ok(())
    };
    render(&mut out).map_err(|_| CostExplanationRefusal::Extent)?;
    Ok(out.0)
}
