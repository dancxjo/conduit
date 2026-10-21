use super::{clock::*, evidence::*, fixture};
use conduit_core::*;
use conduit_planner::resource_cost::*;
use conduit_std_host::*;
use serde_json::json;
pub const HORIZON: usize = 1024;
struct Pairs {
    old: Vec<Sample>,
    new: Vec<Sample>,
    epochs: Vec<MonotonicInstant>,
}
fn pairs_parts(
    host: &mut StdHost,
    previous_plan: &Plan,
    candidate_plan: &Plan,
    inputs: &[ExternalForeInput],
    clock: &MonotonicClockIdentity,
    log: &mut Log,
    noisy: bool,
) -> Result<Pairs, Error> {
    let mut result = Pairs {
        old: Vec::with_capacity(3),
        new: Vec::with_capacity(3),
        epochs: Vec::with_capacity(3),
    };
    for i in 0..3 {
        let label = if noisy { "noisy" } else { "persistent" };
        let (previous, next) = if !noisy && i % 2 == 1 {
            let next = submit(host, candidate_plan, inputs, clock, None)?;
            let previous = submit(host, previous_plan, inputs, clock, None)?;
            log.execution(&format!("{label}/{i}/candidate"), &next)?;
            log.execution(&format!("{label}/{i}/previous"), &previous)?;
            (previous, next)
        } else {
            let previous = submit(host, previous_plan, inputs, clock, None)?;
            log.execution(&format!("{label}/{i}/previous"), &previous)?;
            // Four explicitly prequeued pure requests form the middle burst.
            // The queue is finite, serial and owned by this workload issuer.
            let mut backlog = Vec::with_capacity(4);
            let arrival = now(clock)?;
            if noisy && i == 1 {
                for _ in 0..4 {
                    backlog.push(submit(host, previous_plan, inputs, clock, None)?);
                }
            }
            let next = submit(
                host,
                candidate_plan,
                inputs,
                clock,
                (!backlog.is_empty()).then_some(arrival),
            )?;
            for (j, queued) in backlog.iter().enumerate() {
                log.execution(&format!("{label}/{i}/queued-explicit-request/{j}"), queued)?;
            }
            log.execution(&format!("{label}/{i}/candidate"), &next)?;
            (previous, next)
        };
        result.old.push(previous);
        result.new.push(next);
        result.epochs.push(now(clock)?);
    }
    Ok(result)
}
fn source<'a>(
    ad: &'a HostAdvertisement,
    gear: &PlannedGear,
    sign: &'a SignId,
    revision: CostCalibrationRevision<'a>,
    at: &MonotonicInstant,
) -> Result<CostEvidenceSource<'a>, Error> {
    let expires = MonotonicInstant::new(
        at.ticks()
            .checked_add(30_000_000_000)
            .ok_or("window overflow")?,
        at.clock().clone(),
    )
    .map_err(fail)?;
    CostEvidenceSource::bind(
        ad,
        ad.capabilities
            .iter()
            .find(|c| c.capability_id == gear.capability_id)
            .ok_or("offer missing")?,
        None,
        sign,
        revision,
        CostObservationWindow::admit(at.clone(), expires).map_err(fail)?,
    )
    .map_err(fail)
}
fn forecast(s: &Sample) -> Result<FulfillmentCostForecast, Error> {
    let zero =
        Some(CostDurationRange::admit(duration(0)?, duration(0)?, &mut work()).map_err(fail)?);
    // Service is the complete ordinary API call, input validation, output ACK,
    // projection and raw report disposal. No additional setup/transfer is omitted
    // from that window; the one retained eight-byte request was prepared once.
    FulfillmentCostForecast::sequential(
        CostForecastStages {
            setup: zero,
            queue: Some(s.queue.range()),
            service: Some(s.service.range()),
            transfer: zero,
        },
        &mut work(),
    )
    .map_err(fail)
}
struct Evaluation<'a> {
    costs: &'a ReplacementCostFacts<'a>,
    ad: &'a HostAdvertisement,
    revision: CostCalibrationRevision<'a>,
    clock: &'a MonotonicClockIdentity,
    old_report: &'a StdRunReport,
    inputs: &'a [ExternalForeInput],
    demand: CostDemand<'a>,
}
impl Evaluation<'_> {
    fn run(
        &self,
        host: &mut StdHost,
        gate: &mut ReluctantReplanGate,
        samples: &Pairs,
        log: &mut Log,
        label: &str,
    ) -> Result<Option<StdRunReport>, Error> {
        let (previous, candidate) = self.costs.gears();
        let sources = (0..3)
            .map(|i| {
                Ok([
                    source(
                        self.ad,
                        previous,
                        &samples.old[i].terminal,
                        self.revision,
                        &samples.epochs[i],
                    )?,
                    source(
                        self.ad,
                        candidate,
                        &samples.new[i].terminal,
                        self.revision,
                        &samples.epochs[i],
                    )?,
                ])
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let models = (0..3)
            .map(|i| Ok([forecast(&samples.old[i])?, forecast(&samples.new[i])?]))
            .collect::<Result<Vec<_>, Error>>()?;
        let forecasts = sources
            .iter()
            .zip(&models)
            .map(|(s, m)| {
                Ok([
                    CostedForecast::bind(&s[0], self.demand, &m[0]).map_err(fail)?,
                    CostedForecast::bind(&s[1], self.demand, &m[1]).map_err(fail)?,
                ])
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let observations = forecasts
            .iter()
            .map(|p| ReplanCostObservation {
                previous: &p[0],
                candidate: &p[1],
            })
            .collect::<Vec<_>>();
        let now = now(self.clock)?;
        let calibrations = [self.revision];
        let context = CostEvaluationContext {
            host: self.ad,
            calibrations: &calibrations,
            now: &now,
            execution_observations: &[],
        };
        let policy = ReluctantReplanPolicy::admit(
            3,
            duration(100_000)?,
            duration(10_000_000_000)?,
            duration(2_000_000_000)?,
            duration(1_000_000_000)?,
            duration(1_000_000)?,
            HORIZON as u16,
            &mut work(),
        )
        .map_err(fail)?;
        let mut outputs = fixture::Outputs::default();
        let outcome = host
            .run_reluctant_replacement_to(
                gate,
                StdReluctantReplanRequest {
                    previous_report: self.old_report,
                    policy,
                    costs: self.costs,
                    observations: &observations,
                    context: &context,
                    inputs: self.inputs,
                },
                &mut outputs,
                &mut std::io::sink(),
                &mut fixture::NoTimer,
                &mut work(),
            )
            .map_err(fail)?;
        log.row(&json!({"case":"decision","label":label,"account":decision(&outcome.decision)?,"output_count":outputs.0.len()}))?;
        if let Some(report) = &outcome.replacement_report {
            let (receipt, terminal) = project(self.costs.candidate(), report, &outputs)?;
            // Execution through the bridge has no separately sampled response
            // interval here; its actual complete receipt is nevertheless retained.
            log.row(&json!({"case":"accepted-replacement","request":"horizon/0","receipt":receipt,"terminal_sign":terminal}))?;
        }
        Ok(outcome.replacement_report)
    }
}
pub fn run(log: &mut Log, host_id: HostId, boot: BootId) -> Result<(), Error> {
    let clock = identity(host_id.clone(), boot.clone())?;
    log.row(&json!({"case":"registration","horizon":HORIZON,"minimum_observations":3,"minimum_span_ns":"100000","maximum_gap_ns":"2000000000","maximum_window_ns":"10000000000","cooldown_ns":"1000000000","minimum_net_ns":"1000000","maximum_explicit_backlog":4,"calibration_identity":"std/reluctant-replan-measured@1","calibration_revision":1,"clock":clock,"limitations":["one hosted pure Value Gear; no Body/UI automatic policy or physical reboot","clock resolution is not certified accuracy or a future timing guarantee","artifact file I/O is outside per-request response windows; no allocation/RSS or universal speedup claim"]}))?;
    let (mut case, preparation) = measure(&clock, || {
        Ok(fixture::case_with_profile(
            StdHostConfig {
                host_id,
                boot_id: boot,
                offer_generation: OfferGeneration(1),
            },
            true,
        ))
    })?;
    let (inputs, transfer) = measure(&clock, || Ok(fixture::inputs(7)))?;
    let mut old_outputs = fixture::Outputs::default();
    let old = case
        .host
        .run_external_plot_to(
            case.previous.fragments[0].clone(),
            &inputs,
            &mut old_outputs,
            &mut std::io::sink(),
            &mut fixture::NoTimer,
        )
        .map_err(fail)?;
    let (old_projection, _) = project(&case.previous, &old, &old_outputs)?;
    log.row(&json!({"case":"initial","receipt":old_projection}))?;
    let (_, residency) = measure(&clock, || {
        if case.previous.fragments[0].placements[0]
            .resources
            .is_empty()
            && case.candidate.fragments[0].placements[0]
                .resources
                .is_empty()
        {
            Ok(())
        } else {
            Err("resource-bearing residence needs its own observer".into())
        }
    })?;
    let (_, migration) = measure(&clock, || {
        if case.previous.fragments[0].states.is_empty()
            && case.candidate.fragments[0].states.is_empty()
            && old
                .kernel
                .as_ref()
                .is_some_and(|k| k.active_play.plan_id == case.previous.plan_id)
        {
            Ok(())
        } else {
            Err("live/mutable migration unsupported".into())
        }
    })?;
    let (before, continuity) = measure(&clock, || {
        digest(&project(&case.previous, &old, &old_outputs)?.0)
    })?;
    let ad = case.host.advertisement().clone();
    let revision_id: ArtifactId = "std/reluctant-replan-measured@1".into();
    let revision = CostCalibrationRevision::new(&revision_id, 1).map_err(fail)?;
    let old_gear = &case.previous.fragments[0].placements[0];
    let new_gear = &case.candidate.fragments[0].placements[0];
    let phases = [preparation, transfer, residency, migration, continuity];
    let stages = [
        ReplacementCostStage::Preparation,
        ReplacementCostStage::Transfer,
        ReplacementCostStage::Residency,
        ReplacementCostStage::Migration,
        ReplacementCostStage::Continuity,
    ];
    let signs: Vec<SignId> = (0..5)
        .map(|i| format!("sign/{}/replacement-stage/{i}", ad.boot_id.as_str()).into())
        .collect();
    let sources = (0..5)
        .map(|i| {
            source(
                &ad,
                if i >= 3 { old_gear } else { new_gear },
                &signs[i],
                revision,
                phases[i].completed(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let components = (0..5)
        .map(|i| {
            ReplacementCostComponent::bind(
                stages[i],
                &sources[i],
                if i >= 3 {
                    &case.previous
                } else {
                    &case.candidate
                },
                &phases[i],
                None,
            )
            .map_err(fail)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let calibration = [revision];
    let at = now(&clock)?;
    let context = CostEvaluationContext {
        host: &ad,
        calibrations: &calibration,
        now: &at,
        execution_observations: &[],
    };
    let facts = ReplacementCostFacts::admit(
        &case.previous,
        old_gear,
        &case.candidate,
        new_gear,
        core::array::from_fn(|i| Some(&components[i])),
        &[],
        &[context],
        &mut work(),
    )
    .map_err(fail)?;
    log.row(&json!({"case":"definition","host":ad,"previous_plan":case.previous,"candidate_plan":case.candidate,"input_bytes":inputs[0].bytes,"old_receipt_sha256":before,"overhead":interval(facts.sequential_overhead())?,"stages":(0..5).map(|i|Ok(json!({"stage":format!("{:?}",stages[i]),"sign_id":signs[i],"source":attribution(&CostSourceAttribution::from_source(&sources[i])),"plan_id":if i>=3 {&case.previous.plan_id} else {&case.candidate.plan_id},"measurement":observation(&phases[i])?,"disposition":if i==2 {"no external resource residence; actual applicability check measured"} else if i==3 {"completed pure old Play; no mutable state migration; actual boundary check measured"} else {"performed"}}))).collect::<Result<Vec<_>,Error>>()?}))?;
    let payload = ValuePayload {
        value_kind: case.authored.expanded.gears[0].inputs[0].value_kind.clone(),
        encoded: inputs[0].bytes.clone(),
    };
    let cost_inputs = [CostInput {
        port: &case.authored.expanded.gears[0].inputs[0].port_id,
        value: &payload,
    }];
    let demand = CostDemand::admit(&case.authored.expanded.gears[0], &cost_inputs).map_err(fail)?;
    // Borrow Plans separately from the mutable Host while retaining old outcomes.
    let mut gate = ReluctantReplanGate::default();
    let noisy = pairs_parts(
        &mut case.host,
        &case.previous,
        &case.candidate,
        &inputs,
        &clock,
        log,
        true,
    )?;
    let evaluation = Evaluation {
        costs: &facts,
        ad: &ad,
        revision,
        clock: &clock,
        old_report: &old,
        inputs: &inputs,
        demand,
    };
    let noisy_result = evaluation.run(&mut case.host, &mut gate, &noisy, log, "noisy")?;
    if noisy_result.is_some() {
        return Err("controlled queued load unexpectedly recommended replacement".into());
    }
    let persistent = pairs_parts(
        &mut case.host,
        &case.previous,
        &case.candidate,
        &inputs,
        &clock,
        log,
        false,
    )?;
    let replacement = evaluation.run(&mut case.host, &mut gate, &persistent, log, "persistent")?;
    let accepted = replacement.is_some();
    if accepted {
        if evaluation
            .run(
                &mut case.host,
                &mut gate,
                &persistent,
                log,
                "already-recommended",
            )?
            .is_some()
        {
            return Err("old recommendation retriggered".into());
        }
        for i in 1..HORIZON {
            let sample = submit(&mut case.host, &case.candidate, &inputs, &clock, None)?;
            log.execution(&format!("horizon/{i}"), &sample)?;
        }
    }
    if !accepted {
        // The finite workload exists independently of the profitability result.
        // Keep executing the ordinary reference Plan when replacement is refused.
        for i in 0..HORIZON {
            let sample = submit(&mut case.host, &case.previous, &inputs, &clock, None)?;
            log.execution(&format!("reference-horizon/{i}"), &sample)?;
        }
    }
    let after = digest(&project(&case.previous, &old, &old_outputs)?.0)?;
    if after != before || !verify_plan(&case.previous) || !verify_plan(&case.candidate) {
        return Err("old custody or immutable Plans changed".into());
    }
    log.row(&json!({"case":"summary","persistent_replacement_accepted":accepted,"executed_horizon_requests":HORIZON,"selected_plan":if accepted {"candidate"} else {"previous"},"old_receipt_before":before,"old_receipt_after":after,"source_scope":"actual installed same-meaning Value execution; local identified producer observations"}))?;
    Ok(())
}
