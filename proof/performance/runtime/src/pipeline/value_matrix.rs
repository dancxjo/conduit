//! Eligible Value reference/tape comparisons; Flow and combined refusals stay exact.
use super::domain::observation::Outputs;
use super::*;
fn value_source(gears: usize) -> String {
    source(gears).replace("U64...|", "U64")
}
fn prepare_value(gears: usize, boot: &str, specialized: bool) -> Result<Prepared, Box<dyn Error>> {
    prepare_queue_mode(
        gears,
        &value_source(gears),
        boot,
        if specialized {
            BackProfile::ScalarTape
        } else {
            BackProfile::Hosted
        },
        false,
        1,
        8,
    )
}
fn eligibility(gears: usize) -> Result<Value, Box<dyn Error>> {
    let flow = match prepare_queue_mode(
        gears,
        &source(gears),
        "boot/matrix-flow-admission",
        BackProfile::ScalarTape,
        false,
        1,
        8,
    ) {
        Err(e) => e.to_string(),
        Ok(_) => {
            return Err("Flow specialization became eligible: measure it before publishing".into())
        }
    };
    let combined =
        if gears > 1 {
            match prepare_queue_mode(
                gears,
                &value_source(gears),
                "boot/matrix-combined-admission",
                BackProfile::ScalarTape,
                true,
                1,
                8,
            ) {
                Err(e) => Some(e.to_string()),
                Ok(_) => return Err(
                    "combined specialization/fusion became eligible: measure it before publishing"
                        .into(),
                ),
            }
        } else {
            None
        };
    Ok(
        json!({"flow_specialization":{"eligible":false,"refusal":flow},"value_combined_fusion":{"eligible":false,"refusal":combined},"scope":"installed scalar tape requires Value Fores; installed pure-chain fusion requires its reviewed U64 Flow profile; no eligibility widened for the benchmark; singleton has no fusion region"}),
    )
}
struct Batch {
    reports: Vec<conduit_std_host::StdRunReport>,
    outputs: Vec<Outputs>,
    phases: Vec<Value>,
    metrics: timing::Metrics,
}
fn batch(prepared: &mut Prepared, count: usize) -> Result<Batch, Box<dyn Error>> {
    let fragments = (0..count)
        .map(|_| prepared.plan.fragments[0].clone())
        .collect::<Vec<_>>();
    let mut outputs = (0..count).map(|_| Outputs::new(1)).collect::<Vec<_>>();
    let mut phases = (0..count)
        .map(|_| phase_observation::PhaseObserver::default())
        .collect::<Vec<_>>();
    let mut reports = Vec::with_capacity(count);
    let (result, metrics) = timing::measure(|| -> Result<(), String> {
        for (i, fragment) in fragments.into_iter().enumerate() {
            outputs[i].begin();
            phases[i].begin();
            let result = prepared.host.run_external_plot_to(
                fragment,
                &prepared.inputs[i..i + 1],
                &mut outputs[i],
                &mut std::io::sink(),
                &mut phases[i],
            );
            phases[i].end();
            reports.push(result?);
        }
        Ok(())
    });
    result?;
    let phases = phases
        .iter()
        .map(|p| p.report().map(|p| json!(p)))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Batch {
        reports,
        outputs,
        phases,
        metrics,
    })
}
fn validate(
    reports: &[conduit_std_host::StdRunReport],
    outputs: &[Outputs],
) -> Result<(String, Vec<Value>), Box<dyn Error>> {
    let mut canonical = Vec::new();
    let mut plays = std::collections::BTreeSet::new();
    let mut rows = Vec::new();
    for (i, (report, out)) in reports.iter().zip(outputs).enumerate() {
        domain::observation::completed(report)?;
        if out.values.len() != 1 {
            return Err("Value matrix output count differs".into());
        }
        let value = &out.values[0];
        if value.front_port_id.as_str() != "output"
            || value.sequence != 0
            || value.track != ConnectionTrack::Payload
            || value.value_kind.as_str() != "value/u64"
            || value.bytes != (i as u64 + 32).to_le_bytes()
        {
            return Err("Value matrix meaning differs".into());
        }
        canonical.extend_from_slice(&value.bytes);
        let k = report.kernel.as_ref().ok_or("Value matrix kernel absent")?;
        if !plays.insert(k.active_play_id.clone())
            || k.value_allocation_capacity_before != k.value_allocation_capacity_after
            || k.kernel_sign
                .iter()
                .filter(|e| e.kind == conduit_kernel::KernelEventKind::RemoteValueDelivered)
                .count()
                != 1
        {
            return Err("Value matrix custody/Signs differ".into());
        }
        rows.push(json!({"input_index":i,"active_play":k.active_play,"output":out.report()?,"observations":report.observations,"signs":domain::observation::signs(k),"decisions":k.decisions,"logical_invocations":k.logical_invocations,"value_capacity_before":k.value_allocation_capacity_before,"value_capacity_after":k.value_allocation_capacity_after}));
    }
    Ok((artifact::digest(&canonical), rows))
}
pub(super) fn run(
    config: &Config,
    raw: &mut artifact::Raw,
    summaries: &mut Vec<Value>,
) -> Result<(), Box<dyn Error>> {
    let identity = artifact::digest(config.output.to_string_lossy().as_bytes());
    for gears in [1, 8, 32] {
        let source = value_source(gears);
        artifact::new_file(
            &config
                .output
                .join("sources")
                .join(format!("value-chain-{gears}.plot")),
        )?
        .write_all(source.as_bytes())?;
        let eligibility = eligibility(gears)?;
        for repetition in 0..config.repetitions {
            let mut parity = None;
            for specialized in if repetition % 2 == 0 {
                [false, true]
            } else {
                [true, false]
            } {
                let mode = if specialized {
                    "value-scalar-specialized"
                } else {
                    "value-reference"
                };
                let case = format!("value-chain-{gears}-{mode}");
                let mut prepared = prepare_value(
                    gears,
                    &format!("boot/runtime-value-matrix/{identity}/{gears}/{repetition}/{mode}"),
                    specialized,
                )?;
                artifact::json(
                    &config
                        .output
                        .join("sources")
                        .join(format!("{case}-{repetition}-plan.json")),
                    &prepared.plan,
                )?;
                artifact::json(
                    &config
                        .output
                        .join("sources")
                        .join(format!("{case}-{repetition}-host.json")),
                    prepared.host.advertisement(),
                )?;
                let Batch {
                    reports,
                    outputs,
                    phases,
                    metrics,
                } = batch(&mut prepared, ITEMS)?;
                let (digest, plays) = validate(&reports, &outputs)?;
                if parity.as_ref().is_some_and(|p| p != &digest) {
                    return Err("Value reference/tape parity differs".into());
                }
                parity = Some(digest.clone());
                raw.row(&json!({"case":case,"mode":mode,"phase":"ordinary-value-matrix","repetition":repetition,"gears":gears,"value_plays":ITEMS,"source_sha256":artifact::digest(source.as_bytes()),"checked_plot_id":prepared.plan.checked_plot_id,"full_host_call":metrics,"per_play_phase_wall":phases,"plays":plays,"ordered_output_sha256":digest,"eligibility":eligibility,"cold_phases":prepared.phases,
                    "scope":"64 separate ordinary Value Plays with one input/output each, reused sealed Plan/Host with distinct increasing Play identities; both modes have identical authored Source, frames, semantics and protocol; each Play includes reservation, Back preparation, required Signs and report translation; no continuous-Flow or steady evaluator throughput claim; compiled Source comparison does not widen installed profile eligibility"}))?;
                summaries.push(json!({"case":case,"mode":mode,"repetition":repetition,"full_host_call":metrics}));
                eprintln!("validated {case} repetition {}", repetition + 1);
            }
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_value_reference_and_tape_preserve_work_and_keep_unsupported_profile_pairs_refused(
    ) -> Result<(), Box<dyn Error>> {
        let _measurement = crate::allocation::TEST_MEASUREMENT.lock().unwrap();
        for gears in [1, 8, 32] {
            eligibility(gears)?;
            let mut prior = None;
            for specialized in [false, true] {
                let mut prepared = prepare_value(
                    gears,
                    &format!("boot/value-conformance/{gears}/{specialized}"),
                    specialized,
                )?;
                if specialized {
                    assert!(prepared.plan.fragments[0]
                        .placements
                        .iter()
                        .all(|p| p.implementation_id.as_str()
                            == conduit_std_offers::PURE_SCALAR_TAPE_STD_IMPLEMENTATION));
                }
                let Batch {
                    reports, outputs, ..
                } = batch(&mut prepared, 4)?;
                let (digest, _) = validate(&reports, &outputs)?;
                if let Some(prior) = prior {
                    assert_eq!(prior, digest);
                }
                prior = Some(digest);
            }
        }
        Ok(())
    }
}
