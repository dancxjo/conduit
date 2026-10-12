//! Bounded historical explanation of an ordinary shared-pool cost decision.
use crate::cost_explanation::{exact, BoundedText, CostExplanationRefusal};
use alloc::string::String;
use conduit_core::{Plan, PoolCostSelectionAccount, RationalQuantity};
use core::fmt::{self, Write};

/// Validate retained Plan/source correlation before presenting the decision.
/// This neither ranks candidates nor authenticates observations or admits work.
pub fn render_pool_cost_selection(
    plan: &Plan,
    account: &PoolCostSelectionAccount,
) -> Result<String, CostExplanationRefusal> {
    account.validate(plan).map_err(|reason| match reason {
        conduit_core::PoolCostAccountRefusal::Extent => CostExplanationRefusal::Extent,
        _ => CostExplanationRefusal::InconsistentSelection,
    })?;
    let mut out = BoundedText(String::new());
    render(account, &mut out).map_err(|_| CostExplanationRefusal::Extent)?;
    Ok(out.0)
}

fn render(account: &PoolCostSelectionAccount, out: &mut BoundedText) -> fmt::Result {
    let evidence = &account.evidence;
    writeln!(
        out,
        "historical shared-pool cost selection; prediction is not a deadline or capacity grant"
    )?;
    writeln!(
        out,
        "plan={} pool={} operation={} selection_sign={} disposition={:?} selected={:?} choice={:?}",
        evidence.plan_id.as_str().escape_default(),
        evidence.pool_id.as_str().escape_default(),
        evidence.operation_id.as_str().escape_default(),
        evidence.sign_id.as_str().escape_default(),
        evidence.disposition,
        evidence.selected_realization,
        account.choice
    )?;
    write!(out, "demand_identity=")?;
    for byte in account.demand {
        write!(out, "{byte:02x}")?;
    }
    writeln!(
        out,
        " (opaque operation correlation; request content omitted)"
    )?;
    for sign in &evidence.observation_sign_ids {
        writeln!(out, "observation_sign={}", sign.as_str().escape_default())?;
    }
    for candidate in &account.candidates {
        writeln!(
            out,
            "candidate={} hard_admission={:?} cost={:?}",
            candidate.realization, candidate.admission, candidate.cost
        )?;
    }
    for source in &account.sources {
        writeln!(
            out,
            "source realization={} availability={:?} host={} boot={} generation={} host_profile={}",
            source.realization,
            source.availability,
            source.host.as_str().escape_default(),
            source.boot.as_str().escape_default(),
            source.generation.0,
            source.host_profile.as_str().escape_default()
        )?;
        writeln!(out, "capability={} kind={} kind_contract={} implementation={} artifact={} execution_profile={} resource={:?}",
            source.capability.as_str().escape_default(), source.kind.as_str().escape_default(),
            source.kind_contract.as_str().escape_default(), source.implementation.as_str().escape_default(),
            source.artifact.as_str().escape_default(), source.execution_profile.as_str().escape_default(), source.resource)?;
        writeln!(
            out,
            "sign={} supporting_signs={:?} observed={:?} expires={:?} evaluated_at={:?}",
            source.sign.as_str().escape_default(),
            source.supporting_signs,
            source.observed,
            source.expires,
            source.evaluated_at
        )?;
        writeln!(out, "cached_calibration={:?} evaluated_calibration={:?} expected_execution={:?} current_execution={:?}",
            source.calibration, source.evaluated_calibration, source.expected_execution, source.current_execution)?;
        write!(out, "original_time=")?;
        quantity(source.duration.source(), out)?;
        write!(out, " tolerance=")?;
        quantity(source.duration.tolerance(), out)?;
        let projection = source.duration.projection();
        write!(out, "\nconverted_time=")?;
        quantity(projection.source(), out)?;
        writeln!(
            out,
            " enclosure={:?} upper_nanoseconds={}",
            projection.target(),
            source.duration.upper_nanoseconds()
        )?;
        write!(out, "lower_outward_error=")?;
        exact(projection.error().lower_distance(), out)?;
        write!(out, " upper_outward_error=")?;
        exact(projection.error().upper_distance(), out)?;
        write!(out, " converted_tolerance=")?;
        quantity(projection.maximum_outward_error(), out)?;
        writeln!(out)?;
        for fact in projection.facts() {
            use conduit_core::projection::ProjectionFact;
            match fact {
                ProjectionFact::Preserved { obligation } => writeln!(
                    out,
                    "fidelity=Preserved obligation={}",
                    obligation.as_str().escape_default()
                )?,
                ProjectionFact::Transformed { obligation, law } => writeln!(
                    out,
                    "fidelity=Transformed obligation={} law={}",
                    obligation.as_str().escape_default(),
                    law.as_str().escape_default()
                )?,
                ProjectionFact::Lost {
                    obligation, class, ..
                } => writeln!(
                    out,
                    "fidelity=Lost obligation={} class={class:?} (outward error retained above)",
                    obligation.as_str().escape_default()
                )?,
            }
        }
    }
    Ok(())
}

fn quantity(value: RationalQuantity, out: &mut impl Write) -> fmt::Result {
    exact(value.coordinate(), out)?;
    write!(
        out,
        " unit={:?} dimension={:?} role={:?}",
        value.unit(),
        value.dimension(),
        value.role()
    )
}
