//! Instrumented real local provider operations; no Body/Play migration claim.
use super::fixture::{provider, provider_with_handle, Cases};
use crate::timing;
use conduit_core::*;
use conduit_planner::{proof::resource_frame::FRAME_BYTES, resource_cost::*};
use conduit_std_host::hosted_resource::HostedResourceGeneration;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::error::Error;
const STAGES: [ReplacementCostStage; 5] = [
    ReplacementCostStage::Preparation,
    ReplacementCostStage::Transfer,
    ReplacementCostStage::Residency,
    ReplacementCostStage::Migration,
    ReplacementCostStage::Continuity,
];
fn fail<E: core::fmt::Debug>(error: E) -> Box<dyn Error> {
    format!("{error:?}").into()
}
fn tick() -> Result<u64, Box<dyn Error>> {
    #[cfg(target_os = "linux")]
    {
        let mut time = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut time) } != 0 {
            return Err("monotonic clock unavailable".into());
        }
        Ok(u64::try_from(time.tv_sec)?
            .checked_mul(1_000_000_000)
            .and_then(|n| n.checked_add(u64::try_from(time.tv_nsec).ok()?))
            .ok_or("clock extent")?)
    }
    #[cfg(not(target_os = "linux"))]
    Err("resource-cost measurements currently support Linux CLOCK_MONOTONIC only".into())
}
fn resolution() -> Result<u64, Box<dyn Error>> {
    #[cfg(target_os = "linux")]
    {
        let mut time = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        if unsafe { libc::clock_getres(libc::CLOCK_MONOTONIC, &mut time) } != 0 {
            return Err("clock resolution unavailable".into());
        }
        let value = u64::try_from(time.tv_sec)?
            .checked_mul(1_000_000_000)
            .and_then(|n| n.checked_add(u64::try_from(time.tv_nsec).ok()?))
            .ok_or("resolution extent")?;
        if value == 0 {
            return Err("clock resolution unknown".into());
        }
        Ok(value)
    }
    #[cfg(not(target_os = "linux"))]
    Err("Linux clock required".into())
}
struct Phase {
    metrics: timing::Metrics,
    measured: CostMeasuredDuration,
}
fn phase<T>(
    clock: &MonotonicClockIdentity,
    operation: impl FnOnce() -> Result<T, Box<dyn Error>>,
) -> Result<(T, Phase), Box<dyn Error>> {
    let (result, metrics) = timing::measure(|| {
        let start = tick()?;
        let value = operation()?;
        let end = tick()?;
        Ok::<_, Box<dyn Error>>((value, start, end))
    });
    let (value, start, end) = result?;
    let measured = CostMeasuredDuration::measure(
        &MonotonicInstant::new(start, clock.clone()).map_err(fail)?,
        &MonotonicInstant::new(end, clock.clone()).map_err(fail)?,
        &mut ExactNumericWork::new(100_000_000),
    )
    .map_err(fail)?;
    Ok((value, Phase { metrics, measured }))
}
fn interval(range: CostDurationRange) -> Result<Value, Box<dyn Error>> {
    let ratio = |value: CostDuration| -> Result<Value, Box<dyn Error>> {
        let (n, d) = value.seconds().checked_unsigned_ratio().map_err(fail)?;
        Ok(json!({"numerator":n.to_string(),"denominator":d.to_string()}))
    };
    Ok(
        json!({"unit":"s","role":"linear","lower":ratio(range.lower())?,"upper":ratio(range.upper())?}),
    )
}
fn attribution(source: &CostEvidenceSource<'_>) -> Value {
    json!({"host_id":source.host().host_id,"boot_id":source.host().boot_id,
        "offer_generation":source.host().offer_generation,"host_profile":source.host().profile,
        "capability":source.offer().capability_id,"implementation":source.offer().implementation,
        "resource":source.resource(),"sign":source.sign(),
        "calibration_identity":source.calibration().identity(),"calibration_revision":source.calibration().revision(),
        "observed_tick":source.window().observed().ticks().to_string(),
        "expires_tick":source.window().expires().ticks().to_string(),"clock":source.window().observed().clock()})
}
fn phase_record(p: &Phase) -> Result<Value, Box<dyn Error>> {
    Ok(
        json!({"metrics":p.metrics,"start_tick":p.measured.started().ticks().to_string(),
        "completed_tick":p.measured.completed().ticks().to_string(),"elapsed_ns":p.measured.elapsed().ticks(),
        "clock":p.measured.completed().clock(),"duration":interval(p.measured.range())?}),
    )
}
fn source<'a>(
    case: &'a Cases,
    plan: &Plan,
    sign: &'a SignId,
    revision: CostCalibrationRevision<'a>,
    measured: &CostMeasuredDuration,
) -> Result<CostEvidenceSource<'a>, Box<dyn Error>> {
    let gear = &plan.fragments[0].placements[0];
    let host = &case.host;
    let observed = measured.completed().clone();
    let expires = MonotonicInstant::new(
        observed
            .ticks()
            .checked_add(60_000_000_000)
            .ok_or("expiry extent")?,
        observed.clock().clone(),
    )
    .map_err(fail)?;
    CostEvidenceSource::bind(
        host,
        host.capabilities
            .iter()
            .find(|c| c.capability_id == gear.capability_id)
            .ok_or("offer")?,
        Some(
            host.resources
                .iter()
                .find(|r| r.pool_id == gear.resources[0].pool_id)
                .ok_or("pool")?,
        ),
        sign,
        revision,
        CostObservationWindow::admit(observed, expires).map_err(fail)?,
    )
    .map_err(fail)
}
#[allow(clippy::too_many_arguments)]
fn admit(
    case: &Cases,
    moved: bool,
    origin: &HostedResourceGeneration,
    target: Option<&HostedResourceGeneration>,
    owner: &ResourceAdmissionOwner,
    revision: CostCalibrationRevision<'_>,
    phases: &[Phase],
    trial: &str,
) -> Result<Value, Box<dyn Error>> {
    let candidate = if moved {
        &case.candidate
    } else {
        &case.previous
    };
    let signs: Vec<SignId> = (0..5)
        .map(|i| format!("{trial}/stage/{i}").into())
        .collect();
    let sources: Vec<_> = signs
        .iter()
        .zip(phases)
        .enumerate()
        .map(|(index, (sign, p))| {
            source(
                case,
                if index == 4 {
                    &case.previous
                } else {
                    candidate
                },
                sign,
                revision,
                &p.measured,
            )
        })
        .collect::<Result<_, _>>()?;
    let components: Vec<_> = (0..5)
        .map(|i| {
            ReplacementCostComponent::bind(
                STAGES[i],
                &sources[i],
                if i == 4 { &case.previous } else { candidate },
                &phases[i].measured,
                Some(if moved && i != 4 {
                    target.ok_or("missing target reference")?.reference()
                } else {
                    origin.reference()
                }),
            )
            .map_err(fail)
        })
        .collect::<Result<_, _>>()?;
    let origin_sign: SignId = format!("{trial}/origin-residence").into();
    let original_source = source(
        case,
        &case.previous,
        &origin_sign,
        revision,
        &phases[2].measured,
    )?;
    let old_gear = &case.previous.fragments[0].placements[0];
    let candidate_gear = &candidate.fragments[0].placements[0];
    let old = CostResidentResource::bind(&original_source, &case.previous, old_gear, owner, origin)
        .map_err(fail)?;
    let new = target
        .map(|target| {
            CostResidentResource::bind(&sources[2], candidate, candidate_gear, owner, target)
                .map_err(fail)
        })
        .transpose()?;
    let mut residences = vec![&old];
    if let Some(new) = &new {
        residences.push(new);
    }
    let now = MonotonicInstant::new(tick()?, phases[0].measured.completed().clock().clone())
        .map_err(fail)?;
    let calibrations = [revision];
    let context = CostEvaluationContext {
        host: &case.host,
        calibrations: &calibrations,
        now: &now,
        execution_observations: &[],
    };
    let facts = ReplacementCostFacts::admit(
        &case.previous,
        old_gear,
        candidate,
        candidate_gear,
        core::array::from_fn(|i| Some(&components[i])),
        &residences,
        &[context],
        &mut ExactNumericWork::new(1_000_000_000),
    )
    .map_err(fail)?;
    let stages = components
        .iter()
        .zip(phases)
        .map(|(c, p)| {
            Ok(
                json!({"stage":format!("{:?}",c.stage()),"attribution":attribution(c.source()),"resource_reference_bytes":c.resource_reference().map(|r|r.encode().expect("validated workload")),"sign":c.source().sign(),
        "plan":c.plan().plan_id,"observation":phase_record(p)?}),
            )
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    let resident_rows = facts.residences().iter().map(|r| json!({"plan":r.plan().plan_id,"placement":r.placement().placement_id,
        "attribution":attribution(r.source()),"binding":r.provider().cost_binding(),"handle":r.provider().cost_handle(),"reference_bytes":r.provider().cost_published_reference().expect("checked live").encode().expect("validated reference"),
        "resident_bytes":r.provider().cost_resident_bytes(),"reservation":r.owner().admissions()})).collect::<Vec<_>>();
    Ok(
        json!({"trial":trial,"mode":if moved {"prepared-local-copy"} else {"retained-local"},"previous_plan":facts.previous().plan_id,
        "candidate_plan":facts.candidate().plan_id,"stages":stages,"sequential_overhead":interval(facts.sequential_overhead())?,
        "residences":resident_rows,"copied_payload_bytes":if moved {FRAME_BYTES} else {0},"origin_resident":true,
        "scope":"actual pre-Play provider preparation/lease/copy/read/retirement and Planner cost admission; no installed Body replacement or remote transport"}),
    )
}

pub fn run(case: &Cases, repetitions: u16) -> Result<Value, Box<dyn Error>> {
    let clock = MonotonicClockIdentity::new(
        case.host.host_id.clone(),
        case.host.boot_id.clone(),
        "linux/CLOCK_MONOTONIC".into(),
        TemporalScale::Nanoseconds,
        resolution()?,
        0,
    )
    .map_err(fail)?;
    let previous = case.previous.clone();
    let candidate = case.candidate.clone();
    let reader: AuthorityGrantId = "resource-cost/reader".into();
    let profile: ArtifactId = "std/resource-replacement-cost@1".into();
    let revision = CostCalibrationRevision::new(&profile, 1).map_err(fail)?;
    let payload = super::fixture::payload();
    let expected_hash = format!("{:x}", Sha256::digest(&payload));
    let observations = case.observations();
    let ((mut origin, mut owner), initial) = phase(&clock, || {
        let mut owner = ResourceAdmissionOwner::new(case.host.clone());
        let gear = &case.previous.fragments[0].placements[0];
        owner
            .admit_planned_placement(case.previous.plan_id.clone(), gear, &observations)
            .map_err(fail)?;
        let provider = provider(&gear.resources[0], &reader, None, None)
            .initialize(&payload)
            .map_err(fail)?;
        Ok((provider, owner))
    })?;
    let mut rows = Vec::new();
    for repetition in 0..repetitions {
        let order = if repetition % 2 == 0 {
            [false, true]
        } else {
            [true, false]
        };
        for moved in order {
            let trial = format!("sign/{}/{repetition}/{moved}", case.host.boot_id.as_str());
            let mut phases = Vec::with_capacity(5);
            let (mut target, preparation) = phase(&clock, || {
                if moved {
                    let gear = &case.candidate.fragments[0].placements[0];
                    owner
                        .admit_planned_placement(
                            case.candidate.plan_id.clone(),
                            gear,
                            &observations,
                        )
                        .map_err(fail)?;
                    Ok(Some(provider_with_handle(
                        &gear.resources[0],
                        &reader,
                        None,
                        None,
                        format!("handle/{trial}/target").into(),
                    )))
                } else {
                    Ok(None)
                }
            })?;
            phases.push(preparation);
            let (lease, transfer) = phase(&clock, || {
                let lease = origin.acquire(&reader).map_err(fail)?;
                let bytes = origin.read(lease).map_err(fail)?;
                if moved {
                    target = Some(
                        target
                            .take()
                            .ok_or("target missing")?
                            .initialize(bytes)
                            .map_err(fail)?,
                    );
                } else {
                    std::hint::black_box(bytes);
                }
                Ok(lease)
            })?;
            phases.push(transfer);
            let (_, residency) = phase(&clock, || {
                let current = target.as_ref().unwrap_or(&origin);
                if current.reference() != origin.reference()
                    || current.payload_residencies() != 1
                    || current.resident_bytes() != FRAME_BYTES
                {
                    return Err("published residence mismatch".into());
                }
                Ok(())
            })?;
            phases.push(residency);
            let (_, migration) = phase(&clock, || {
                origin.release(lease).map_err(fail)?;
                let current = target.as_mut().unwrap_or(&mut origin);
                let check = current.acquire(&reader).map_err(fail)?;
                if format!("{:x}", Sha256::digest(current.read(check).map_err(fail)?))
                    != expected_hash
                {
                    return Err("moved bytes changed".into());
                }
                current.release(check).map_err(fail)?;
                Ok(())
            })?;
            phases.push(migration);
            let (_, continuity) = phase(&clock, || {
                let check = origin.acquire(&reader).map_err(fail)?;
                if format!("{:x}", Sha256::digest(origin.read(check).map_err(fail)?))
                    != expected_hash
                {
                    return Err("old owner changed".into());
                }
                origin.release(check).map_err(fail)?;
                Ok(())
            })?;
            phases.push(continuity);
            let (mut row, admission) = phase(&clock, || {
                admit(
                    case,
                    moved,
                    &origin,
                    target.as_ref(),
                    &owner,
                    revision,
                    &phases,
                    &trial,
                )
            })?;
            row["cost_admission_and_report_construction"] = phase_record(&admission)?;
            let (_, disposal) = phase(&clock, || {
                if moved {
                    owner
                        .release(
                            &case.candidate.plan_id,
                            &case.candidate.fragments[0].placements[0].placement_id,
                        )
                        .map_err(fail)?;
                }
                drop(target);
                Ok(())
            })?;
            row["target_reservation_release_and_disposal"] = phase_record(&disposal)?;
            row["payload_sha256"] = json!(expected_hash);
            rows.push(row);
        }
    }
    let (_, disposal) = phase(&clock, || {
        owner
            .release(
                &case.previous.plan_id,
                &case.previous.fragments[0].placements[0].placement_id,
            )
            .map_err(fail)?;
        if !owner.admissions().is_empty() {
            return Err("reservation leak".into());
        }
        drop(origin);
        drop(owner);
        Ok(())
    })?;
    if case.previous != previous || case.candidate != candidate {
        return Err("cost observation mutated Plan".into());
    }
    Ok(
        json!({"schema":"conduit.resource-replacement-cost@1","clock":clock,"payload_bytes":FRAME_BYTES,"payload_sha256":expected_hash,
        "initial_resident_preparation":phase_record(&initial)?,"final_origin_release_and_disposal":phase_record(&disposal)?,"trials":rows,
        "limitations":["same-process local provider copies only; no installed Body/Play replacement or cross-Host transport", "CLOCK_MONOTONIC resolution only; no certified wall-clock accuracy or independent-process uncertainty", "target is newly prepared for each copied case; retained case reuses the original published owner", "modeled sequential overhead excludes cost-admission/report construction and final disposal, which are recorded separately", "allocator observations include harness work within their named phases, not RSS/hardware traffic"]}),
    )
}
