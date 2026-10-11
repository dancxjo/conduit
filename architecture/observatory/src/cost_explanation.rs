//! Bounded, read-only projection of a completed Planner cost decision.
//! This is historical evidence, never a fresh Offer or resource admission.
use alloc::string::String;
use conduit_core::{ExactRational, EXACT_RATIONAL_MAX_ENCODED_BYTES};
use conduit_planner::{
    resource_cost::{
        CostAvailability, CostDurationRange, CostEvidenceSource, CostRealizationSelection,
        CostSelectionError, MAXIMUM_COST_CANDIDATES,
    },
    RealizationDecisionDisposition,
};
use core::fmt::{self, Write};

pub const MAXIMUM_COST_EXPLANATION_BYTES: usize = 524_288;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CostExplanationRefusal {
    Extent,
    InconsistentSelection,
    NoRetainedDecision,
}

/// Explain a cost-policy refusal while retaining the ordinary hard-admission
/// records. Its tentative ordinary selection is not a cost-admitted winner.
/// Other errors currently lack this account; refuse instead of inventing it.
pub fn render_cost_selection_refusal(
    error: &CostSelectionError,
) -> Result<String, CostExplanationRefusal> {
    let CostSelectionError::NoUsableForecast { realization, costs } = error else {
        return Err(CostExplanationRefusal::NoRetainedDecision);
    };
    if realization.signs.len() > MAXIMUM_COST_CANDIDATES || costs.len() > MAXIMUM_COST_CANDIDATES {
        return Err(CostExplanationRefusal::Extent);
    }
    let mut out = BoundedText(String::new());
    let render = |out: &mut BoundedText| -> fmt::Result {
        writeln!(
            out,
            "historical planner cost policy refused; no cost-admitted winner"
        )?;
        for record in &realization.signs {
            let disposition = match &record.disposition {
                RealizationDecisionDisposition::Selected => {
                    &RealizationDecisionDisposition::Admitted
                }
                other => other,
            };
            writeln!(out, "candidate host={} boot={} generation={} capability={} hard_admission={disposition:?} hard_clause_source={:?}",
                record.host_id.as_str().escape_default(), record.boot_id.as_str().escape_default(),
                record.offer_generation.0, record.capability_id.as_str().escape_default(), record.clause_source)?;
        }
        for cost in costs {
            writeln!(
                out,
                "cost subject host={} capability={} evaluated_at={:?}",
                cost.host_id.as_str().escape_default(),
                cost.capability_id.as_str().escape_default(),
                cost.evaluated_at
            )?;
            if let Some(reason) = &cost.reason {
                writeln!(out, "cost=unusable reason={reason:?}")?;
            } else {
                writeln!(out, "cost=unknown (no numeric substitute)")?;
            }
            writeln!(
                out,
                "submitted={:?} submitted_calibration={:?} (historical attribution)",
                cost.submitted, cost.submitted_calibration
            )?;
            writeln!(out, "current_execution={:?} expected_execution_condition={:?} execution_assumption={:?} (historical attribution)",
                cost.current_execution, cost.expected_execution_condition, cost.submitted_execution_assumption)?;
        }
        Ok(())
    };
    render(&mut out).map_err(|_| CostExplanationRefusal::Extent)?;
    Ok(out.0)
}

/// Render the existing decision and its source account without cloning Host
/// inventories, interpreting raw throughput, reranking, or exposing inputs.
/// Identities are escaped and output growth is bounded outside Play. A caller
/// still owns authorization to reveal Host/Boot/Sign correlation identities.
pub fn render_cost_selection_explanation(
    selection: &CostRealizationSelection<'_>,
) -> Result<String, CostExplanationRefusal> {
    validate_shape(selection)?;
    let mut out = BoundedText(String::new());
    render(selection, &mut out).map_err(|_| CostExplanationRefusal::Extent)?;
    Ok(out.0)
}

fn validate_shape(selection: &CostRealizationSelection<'_>) -> Result<(), CostExplanationRefusal> {
    let records = &selection.realization.signs;
    if records.len() > MAXIMUM_COST_CANDIDATES || selection.costs.len() > MAXIMUM_COST_CANDIDATES {
        return Err(CostExplanationRefusal::Extent);
    }
    let choice = &selection.realization.choice;
    if records
        .iter()
        .filter(|record| record.disposition == RealizationDecisionDisposition::Selected)
        .count()
        != 1
        || !records.iter().any(|record| {
            record.host_id == choice.host_id
                && record.capability_id == choice.capability_id
                && record.disposition == RealizationDecisionDisposition::Selected
        })
    {
        return Err(CostExplanationRefusal::InconsistentSelection);
    }
    for (index, record) in records.iter().enumerate() {
        if records[..index].iter().any(|previous| {
            previous.host_id == record.host_id && previous.capability_id == record.capability_id
        }) {
            return Err(CostExplanationRefusal::InconsistentSelection);
        }
        let cost = selection.costs.iter().find(|cost| {
            *cost.host_id == record.host_id && *cost.capability_id == record.capability_id
        });
        if matches!(
            record.disposition,
            RealizationDecisionDisposition::Rejected(_)
        ) {
            if cost.is_some() {
                return Err(CostExplanationRefusal::InconsistentSelection);
            }
        } else if cost.is_none_or(|cost| {
            cost.selected != (record.disposition == RealizationDecisionDisposition::Selected)
        }) {
            return Err(CostExplanationRefusal::InconsistentSelection);
        }
    }
    for (index, cost) in selection.costs.iter().enumerate() {
        if selection.costs[..index].iter().any(|previous| {
            previous.host_id == cost.host_id && previous.capability_id == cost.capability_id
        }) || !records.iter().any(|record| {
            &record.host_id == cost.host_id && &record.capability_id == cost.capability_id
        }) {
            return Err(CostExplanationRefusal::InconsistentSelection);
        }
        for source in [
            cost.submitted_source,
            cost.submitted_calibration_source,
            cost.submitted_execution_assumption,
            cost.current_execution.map(|sample| sample.source()),
        ]
        .into_iter()
        .flatten()
        {
            if &source.host().host_id != cost.host_id
                || &source.offer().capability_id != cost.capability_id
            {
                return Err(CostExplanationRefusal::InconsistentSelection);
            }
        }
        if let CostAvailability::Known(sample) = cost.availability {
            let source = sample.source();
            if sample
                .validate_for_execution(
                    source.host(),
                    source.calibration(),
                    cost.evaluated_at,
                    cost.current_execution,
                )
                .is_err()
                || cost.expected_execution_condition != sample.expected_execution_condition()
                || cost.submitted_execution_assumption
                    != sample
                        .execution_assumption()
                        .map(|observation| observation.source())
                || cost.submitted_source != Some(source)
                || cost.submitted_calibration_source
                    != sample.calibration().map(|estimate| estimate.source())
            {
                return Err(CostExplanationRefusal::InconsistentSelection);
            }
        }
        if matches!(cost.availability, CostAvailability::Missing) != cost.submitted_source.is_none()
        {
            return Err(CostExplanationRefusal::InconsistentSelection);
        }
    }
    Ok(())
}

fn render(selection: &CostRealizationSelection<'_>, out: &mut impl Write) -> fmt::Result {
    writeln!(
        out,
        "historical planner cost decision; model estimates are not deadlines"
    )?;
    writeln!(
        out,
        "intent={:?} cost_preference={} arithmetic_limb_work={} (not wall time)",
        selection.intent, selection.used_cost_preference, selection.arithmetic_work_spent
    )?;
    for record in &selection.realization.signs {
        writeln!(
            out,
            "candidate host={} boot={} generation={} capability={} implementation={} artifact={} disposition={:?} tie_clause={:?} hard_clause_source={:?} tie_source={:?}",
            record.host_id.as_str().escape_default(),
            record.boot_id.as_str().escape_default(),
            record.offer_generation.0,
            record.capability_id.as_str().escape_default(),
            record.implementation_id.as_str().escape_default(),
            record.artifact_id.as_str().escape_default(),
            record.disposition,
            record.decisive_preference_clause,
            record.clause_source,
            record.decisive_preference_source
        )?;
        let Some(cost) = selection.costs.iter().find(|cost| {
            *cost.host_id == record.host_id && *cost.capability_id == record.capability_id
        }) else {
            writeln!(out, "cost=not-compared (hard refusal retained)")?;
            continue;
        };
        writeln!(out, "evaluated_at={:?} (subject clock)", cost.evaluated_at)?;
        if let Some(submitted) = cost.submitted_source {
            source(submitted, "submitted", out)?;
        }
        if let Some(calibration) = cost.submitted_calibration_source {
            source(calibration, "submitted_calibration", out)?;
        }
        if let Some(observation) = cost.current_execution {
            source(observation.source(), "current_execution", out)?;
            writeln!(
                out,
                "current_execution_condition={:?}",
                observation.condition()
            )?;
        }
        if let Some(assumption) = cost.submitted_execution_assumption {
            source(assumption, "execution_assumption", out)?;
        }
        writeln!(
            out,
            "expected_execution_condition={:?}",
            cost.expected_execution_condition
        )?;
        match &cost.availability {
            CostAvailability::Missing => writeln!(out, "cost=unknown (no numeric substitute)")?,
            CostAvailability::Unusable(reason) => writeln!(out, "cost=unusable reason={reason:?}")?,
            CostAvailability::Known(sample) => {
                let demand = sample.demand();
                writeln!(
                    out,
                    "demand kind={} contract={} finite_inputs={} input_bytes={} (contents withheld)",
                    demand.class().gear().kind_id.as_str().escape_default(),
                    demand.class().gear().kind_contract_revision.as_str().escape_default(),
                    demand.inputs().len(),
                    demand.input_bytes()
                )?;
                if let Some(estimate) = demand.work_estimate() {
                    writeln!(
                        out,
                        "work metric={} revision={} units={} admitted_maximum_units={}",
                        estimate.metric_identity().as_str().escape_default(),
                        estimate.metric_revision(),
                        estimate.units(),
                        estimate.maximum_units()
                    )?;
                }
                if let Some(basis) = sample.service_basis() {
                    stage_range("service_per_work_unit", Some(basis), out)?;
                }
                let stages = sample.forecast().stages();
                for (name, stage) in [
                    ("setup", stages.setup),
                    ("queue", stages.queue),
                    ("service", stages.service),
                    ("transfer", stages.transfer),
                ] {
                    stage_range(name, stage, out)?;
                }
                write!(out, "effective_fulfillment_seconds=[")?;
                exact(sample.lower_seconds(), out)?;
                write!(out, ",")?;
                exact(sample.upper_seconds(), out)?;
                writeln!(out, "]")?;
                if let Some(calibration) = sample.calibration() {
                    let record = calibration.completion_record();
                    writeln!(out, "measured_elapsed={:?}", record.elapsed())?;
                    stage_range("measured", Some(record.measured()), out)?;
                    let (lower, upper) = record.error_bounds();
                    write!(out, "prediction_error_seconds=[")?;
                    exact(lower.coordinate(), out)?;
                    write!(out, ",")?;
                    exact(upper.coordinate(), out)?;
                    writeln!(out, "] (signed measured minus predicted)")?;
                    writeln!(
                        out,
                        "calibration updates={} condition={:?} (mean estimate)",
                        calibration.updates(),
                        calibration.condition()
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn source(source: &CostEvidenceSource<'_>, label: &str, out: &mut impl Write) -> fmt::Result {
    writeln!(out,
        "{label} sign={} host={} boot={} generation={} host_profile={} capability={} execution_profile={} implementation={} artifact={} calibration={} revision={} resource={:?}",
        source.sign().as_str().escape_default(),
        source.host().host_id.as_str().escape_default(),
        source.host().boot_id.as_str().escape_default(),
        source.host().offer_generation.0,
        source.host().profile.as_str().escape_default(),
        source.offer().capability_id.as_str().escape_default(),
        source.offer().implementation.execution_profile_id.as_str().escape_default(),
        source.offer().implementation.implementation_id.as_str().escape_default(),
        source.offer().implementation.artifact_id.as_str().escape_default(),
        source.calibration().identity().as_str().escape_default(),
        source.calibration().revision(),
        source.resource().map(|resource| (&resource.pool_id, &resource.class_id)))?;
    writeln!(
        out,
        "{label} observed={:?} expires={:?} (producer clock; historical window)",
        source.window().observed(),
        source.window().expires()
    )
}

fn stage_range(name: &str, range: Option<CostDurationRange>, out: &mut impl Write) -> fmt::Result {
    write!(out, "stage {name}=")?;
    if let Some(range) = range {
        for (index, duration) in [range.lower(), range.upper()].into_iter().enumerate() {
            if index > 0 {
                write!(out, "..")?;
            }
            exact(duration.original().coordinate(), out)?;
            write!(
                out,
                " {} role={:?}",
                duration.original().unit().symbol().escape_default(),
                duration.original().role()
            )?;
        }
        writeln!(out)
    } else {
        writeln!(out, "unknown")
    }
}

/// Display exact fractions, using decimal magnitudes that fit u128 and hex for
/// larger magnitudes. This formats codec components without projecting a Time
/// quantity into a scalar, rounding, or performing big-number base conversion.
fn exact(value: ExactRational, out: &mut impl Write) -> fmt::Result {
    let mut encoded = [0u8; EXACT_RATIONAL_MAX_ENCODED_BYTES];
    let length = value.encode(&mut encoded).map_err(|_| fmt::Error)?;
    let numerator_length = u16::from_le_bytes([encoded[2], encoded[3]]) as usize;
    if value.is_negative() {
        write!(out, "-")?;
    }
    for (index, bytes) in [
        &encoded[6..6 + numerator_length],
        &encoded[6 + numerator_length..length],
    ]
    .into_iter()
    .enumerate()
    {
        if index > 0 {
            write!(out, "/")?;
        }
        if bytes.len() <= 16 {
            let mut magnitude = [0u8; 16];
            magnitude[..bytes.len()].copy_from_slice(bytes);
            write!(out, "{}", u128::from_le_bytes(magnitude))?;
        } else {
            write!(out, "0x")?;
            for byte in bytes.iter().rev() {
                write!(out, "{byte:02x}")?;
            }
        }
    }
    Ok(())
}

struct BoundedText(String);
impl Write for BoundedText {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if value.len() > MAXIMUM_COST_EXPLANATION_BYTES - self.0.len() {
            return Err(fmt::Error);
        }
        self.0.push_str(value);
        Ok(())
    }
}

#[cfg(test)]
mod bounds_tests {
    use super::*;
    #[test]
    fn exact_fraction_display_never_rounds_or_truncates_large_magnitudes() {
        let mut work = conduit_core::ExactNumericWork::new(100_000_000);
        let negative = ExactRational::ratio(-1, 3, &mut work).unwrap();
        let mut output = String::new();
        exact(negative, &mut output).unwrap();
        assert_eq!(output, "-1/3");
        let large =
            ExactRational::parse_decimal("340282366920938463463374607431768211456", &mut work)
                .unwrap();
        output.clear();
        exact(large, &mut output).unwrap();
        assert_eq!(output, "0x0100000000000000000000000000000000/1");
    }
    #[test]
    fn output_extent_refuses_growth_atomically() {
        let mut out = BoundedText(String::new());
        out.write_str(&alloc::string::String::from("x").repeat(MAXIMUM_COST_EXPLANATION_BYTES))
            .unwrap();
        assert!(out.write_str("x").is_err());
        assert_eq!(out.0.len(), MAXIMUM_COST_EXPLANATION_BYTES);
    }
}
