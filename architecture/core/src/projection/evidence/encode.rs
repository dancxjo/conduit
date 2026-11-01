use super::*;
use crate::claims::ClaimScore;
type E = ProjectionEvidenceRefusal;
type W<'a> = ProjectionEvidenceWriter<'a>;
pub(super) fn report<D: ProjectionDomain, C: ProjectionEvidenceCodec<D>>(
    report: &ProjectionReport<'_, D>,
    codec: &C,
    limits: ProjectionEvidenceLimits,
    out: &mut W<'_>,
) -> Result<(), E> {
    let input = report.input();
    let basis = input.basis;
    out.raw(MAGIC)?;
    out.text(codec.identity().as_str())?;
    out.text(report.source_contract().as_str())?;
    out.text(report.target_contract().as_str())?;
    out.text(report.policy().as_str())?;
    out.raw(
        &report
            .policy_configuration_digest()
            .ok_or(E::PolicyConfigurationUnavailable)?,
    )?;
    out.text(basis.report.as_str())?;
    out.text(basis.source.as_str())?;
    out.optional_text(basis.target.map(ProjectionText::as_str))?;
    out.text(basis.projector.as_str())?;
    out.text(basis.requested_route.as_str())?;
    match basis.boundary {
        None => out.tag(0)?,
        Some(b) => {
            out.tag(1)?;
            out.text(b.expanded_form.as_str())?;
            out.text(b.subject.as_str())?;
        }
    }
    out.field(limits.policy_bytes, |w| codec.policy(w))?;
    out.field(limits.source_bytes, |w| codec.source(input.source, w))?;
    match input.target {
        None => out.tag(0)?,
        Some(target) => {
            out.tag(1)?;
            out.field(limits.target_bytes, |w| codec.target(target, w))?;
        }
    }
    out.count(input.facts.len())?;
    for fact in input.facts {
        match fact {
            ProjectionFact::Preserved { obligation } => {
                out.tag(0)?;
                out.text(obligation.as_str())?;
            }
            ProjectionFact::Transformed { obligation, law } => {
                out.tag(1)?;
                out.text(obligation.as_str())?;
                out.text(law.as_str())?;
            }
            ProjectionFact::Lost {
                obligation,
                class,
                detail,
                native_fact,
            } => {
                out.tag(2)?;
                out.text(obligation.as_str())?;
                out.tag(loss(*class))?;
                out.field(limits.detail_bytes, |w| codec.detail(detail, w))?;
                out.optional_text(native_fact.map(ProjectionText::as_str))?;
            }
        }
    }
    out.count(input.native.len())?;
    for native in input.native {
        for text in [
            native.identity,
            native.contract,
            native.provider,
            native.encoding,
        ] {
            out.text(text.as_str())?;
        }
        out.bytes(native.bytes)?;
    }
    out.count(input.scores.len())?;
    for item in input.scores {
        match item {
            ProjectionScore::Native { fact, score: value } => {
                out.tag(0)?;
                out.text(fact.as_str())?;
                score(*value, out)?;
            }
            ProjectionScore::Normalized {
                fact,
                original,
                projected,
                policy,
            } => {
                out.tag(1)?;
                out.text(fact.as_str())?;
                score(*original, out)?;
                score(*projected, out)?;
                out.text(policy.as_str())?;
            }
        }
    }
    out.count(input.admitted.len())?;
    for item in input.admitted {
        realization(*item, out)?;
    }
    out.count(input.attempts.len())?;
    for item in input.attempts {
        realization(item.realization, out)?;
        mechanism(item.outcome, out)?;
        out.optional_text(item.fallback_reason.map(ProjectionText::as_str))?;
    }
    match input.selected_attempt {
        None => out.tag(0)?,
        Some(index) => {
            out.tag(1)?;
            out.count(index)?;
        }
    }
    mechanism(input.mechanism, out)?;
    out.count(input.diagnostics.len())?;
    for item in input.diagnostics {
        out.text(item.as_str())?;
    }
    let summary = report.summary();
    match summary.disposition {
        ProjectionDisposition::Completed(ProjectionFidelity::Exact) => out.tag(0)?,
        ProjectionDisposition::Completed(ProjectionFidelity::PermittedLossy) => out.tag(1)?,
        ProjectionDisposition::Insufficient => out.tag(2)?,
        ProjectionDisposition::Refused(info) => {
            out.tag(3)?;
            out.raw(&info.encode())?;
        }
        ProjectionDisposition::Failed(info) => {
            out.tag(4)?;
            out.raw(&info.encode())?;
        }
        ProjectionDisposition::Cancelled(info) => {
            out.tag(5)?;
            out.raw(&info.encode())?;
        }
    }
    out.count(summary.preserved)?;
    out.count(summary.transformed)?;
    for count in summary.losses {
        out.count(count)?;
    }
    Ok(())
}
fn score(score: ClaimScore<'_>, out: &mut W<'_>) -> Result<(), E> {
    out.text(score.scale().as_str())?;
    out.optional_text(score.calibration().map(|s| s.as_str()))?;
    out.text(score.producer().as_str())?;
    let (min, max) = score.bounds();
    out.i64(min)?;
    out.i64(max)?;
    out.i64(score.value())
}
fn realization(value: ProjectionRealization<'_>, out: &mut W<'_>) -> Result<(), E> {
    for text in [
        value.plan.as_str(),
        value.host.as_str(),
        value.boot.as_str(),
        value.back.as_str(),
        value.artifact.as_str(),
    ] {
        out.text(text)?;
    }
    Ok(())
}
fn mechanism(value: ProjectionMechanism, out: &mut W<'_>) -> Result<(), E> {
    match value {
        ProjectionMechanism::Completed => out.tag(0),
        ProjectionMechanism::Refused(info) => {
            out.tag(1)?;
            out.raw(&info.encode())
        }
        ProjectionMechanism::Failed(info) => {
            out.tag(2)?;
            out.raw(&info.encode())
        }
        ProjectionMechanism::Cancelled(info) => {
            out.tag(3)?;
            out.raw(&info.encode())
        }
    }
}
fn loss(value: ProjectionLoss) -> u8 {
    match value {
        ProjectionLoss::Unrepresentable => 0,
        ProjectionLoss::Unrecognized => 1,
        ProjectionLoss::Precision => 2,
        ProjectionLoss::Range => 3,
        ProjectionLoss::Cardinality => 4,
        ProjectionLoss::Ordering => 5,
        ProjectionLoss::TemporalFinality => 6,
        ProjectionLoss::Identity => 7,
        ProjectionLoss::Provenance => 8,
        ProjectionLoss::UnsupportedVariant => 9,
        ProjectionLoss::Truncation => 10,
        ProjectionLoss::Approximation => 11,
    }
}
