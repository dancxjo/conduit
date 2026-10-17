//! Matched std local/fused trials through the ordinary Plan/Play entrance.
use super::*;

pub(super) fn run(
    config: &Config,
    raw: &mut artifact::Raw,
    summaries: &mut Vec<Value>,
) -> Result<(), Box<dyn Error>> {
    let run_identity = artifact::digest(config.output.to_string_lossy().as_bytes());
    for gears in [1, 8, 32] {
        let authored = source(gears);
        for repetition in 0..config.repetitions {
            let order = if repetition % 2 == 0 {
                [false, true]
            } else {
                [true, false]
            };
            let mut oracle = None;
            let mut decisions = [0; 2];
            for fused in order {
                let mode = if fused { "fused" } else { "local-unfused" };
                let case = format!("pure-chain-{gears}-{mode}");
                let boot =
                    format!("boot/runtime-fusion/{run_identity}/{gears}/{repetition}/{mode}");
                let mut prepared = prepare_mode(gears, &authored, &boot, true, fused)?;
                let fragment = prepared.plan.fragments[0].clone();
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
                let mut collector = Collector::new();
                let started = Instant::now();
                let (report, metrics) = timing::measure(|| {
                    prepared.host.run_external_plot_sequence_to(
                        fragment,
                        &prepared.inputs,
                        &mut collector,
                        &mut std::io::sink(),
                        &mut NoTimer,
                    )
                });
                let report = report?;
                let digest = validate(&collector.values)?;
                if let Some(expected) = &oracle {
                    if expected != &digest {
                        return Err("fused/unfused ordered output mismatch".into());
                    }
                } else {
                    oracle = Some(digest.clone());
                }
                let kernel = report.kernel.ok_or("fusion kernel receipt absent")?;
                if kernel.value_allocation_capacity_before != kernel.value_allocation_capacity_after
                {
                    return Err("fusion value store grew during Play".into());
                }
                let mandatory = kernel
                    .kernel_sign
                    .iter()
                    .filter(|e| e.kind == conduit_kernel::KernelEventKind::RemoteValueDelivered)
                    .count();
                if mandatory != ITEMS {
                    return Err("fusion mandatory output Sign missing".into());
                }
                decisions[usize::from(fused)] = kernel.decisions;
                let first = collector.first.ok_or("first fused output missing")?;
                let last = collector.last.ok_or("last fused output missing")?;
                let row = json!({
                    "case":case,"phase":"finite-ordinary-fusion-pipeline","mode":mode,"repetition":repetition,"gears":gears,"values":ITEMS,
                    "source_sha256":artifact::digest(authored.as_bytes()),"checked_plot_id":prepared.plan.checked_plot_id.as_str(),
                    "ordered_output_sha256":digest,"full_host_call":metrics,"cold_phases":prepared.phases,"planning_selection":prepared.selection_evidence,
                    "first_acknowledgement_from_call_start_ns":timing::ns(first.duration_since(started)),
                    "first_to_last_acknowledgement_ns":timing::ns(last.duration_since(first)),
                    "completion_window_useful_outputs_per_second":(ITEMS-1) as f64 / last.duration_since(first).as_secs_f64(),
                    "post_first_output_allocations":collector.allocation_window()?,
                    "kernel_decisions":kernel.decisions,"physical_regions":prepared.plan.fragments[0].execution_fusions.len(),
                    "logical_gear_count":prepared.plan.fragments[0].placements.len(),"logical_internal_cord_count":prepared.plan.fragments[0].connections.len(),
                    "mandatory_output_sign_count":mandatory,"active_play":kernel.active_play,
                    "value_capacity_before":kernel.value_allocation_capacity_before,"value_capacity_after":kernel.value_allocation_capacity_after,
                    "work_accounting":{"logical_evaluations":gears*ITEMS,"intermediate_materializations":(gears-1)*ITEMS,"intermediate_canonical_bytes":(gears-1)*ITEMS*8,
                        "basis":"exact fixed fixture work; all intermediate Cords and canonical codecs retained; no avoided-copy claim", "internal_copy_counter":null},
                    "scope":"full call includes reservation, Back preparation, bounded kernel Play and report translation; completion window includes Host/collector work; std only",
                    "signs":kernel.kernel_sign.iter().map(|e|json!({"sequence":e.sequence,"node":e.node.0,"kind":format!("{:?}",e.kind)})).collect::<Vec<_>>(),
                });
                raw.row(&row)?;
                summaries.push(row);
            }
            if gears > 1 && decisions[1] >= decisions[0] {
                return Err("fusion did not reduce physical scheduler decisions".into());
            }
        }
    }
    Ok(())
}
