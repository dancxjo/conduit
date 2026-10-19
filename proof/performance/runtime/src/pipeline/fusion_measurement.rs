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
            let mut order = vec![("local-unfused", false, false), ("fused", true, false)];
            if gears > 1 {
                order.push(("native-fused", true, true));
            }
            let rotate = usize::from(repetition) % order.len();
            order.rotate_left(rotate);
            let mut oracle = None;
            let mut sign_oracle: Option<Vec<conduit_kernel::KernelEvent>> = None;
            let mut lifecycle_oracle = None;
            let mut invocation_oracle = None;
            let mut decisions = [0; 3];
            artifact::json(
                &config
                    .output
                    .join("sources")
                    .join(format!("pure-chain-{gears}-{repetition}-source.json")),
                &authored,
            )?;
            for (mode, fused, native) in order {
                let case = format!("pure-chain-{gears}-{mode}");
                let boot =
                    format!("boot/runtime-fusion/{run_identity}/{gears}/{repetition}/{mode}");
                let mut prepared = if native {
                    prepare_queue_mode(
                        gears,
                        &authored,
                        &boot,
                        BackProfile::NativeLocal,
                        true,
                        1,
                        8,
                    )?
                } else {
                    prepare_mode(gears, &authored, &boot, true, fused)?
                };
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
                let mut phases = phase_observation::PhaseObserver::default();
                let started = Instant::now();
                let (report, metrics) = timing::measure(|| {
                    phases.begin();
                    let result = prepared.host.run_external_plot_sequence_to(
                        fragment,
                        &prepared.inputs,
                        &mut collector,
                        &mut std::io::sink(),
                        &mut phases,
                    );
                    phases.end();
                    result
                });
                let report = report?;
                let kernel_phase_wall = phases.report()?;
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
                let lifecycle = required_lifecycle(&kernel.kernel_sign);
                if lifecycle.len() != 258 + 2 * gears {
                    return Err("required chain lifecycle evidence count changed".into());
                }
                if let Some(expected) = &lifecycle_oracle {
                    if expected != &lifecycle {
                        return Err("physical profiles changed ordered lifecycle identities".into());
                    }
                } else {
                    lifecycle_oracle = Some(lifecycle);
                }
                if invocation_oracle
                    .replace(kernel.logical_invocations)
                    .is_some_and(|n| n != kernel.logical_invocations)
                {
                    return Err("physical profiles changed logical invocation work".into());
                }
                // Fusion changes scheduler ordering relative to the unfused
                // baseline. Native transport must keep the canonical fused
                // execution's exact sequence and every retained event field.
                if fused {
                    if let Some(expected) = &sign_oracle {
                        if expected != &kernel.kernel_sign {
                            return Err(
                                "native/canonical fused retained Sign sequence changed".into()
                            );
                        }
                    } else {
                        sign_oracle = Some(kernel.kernel_sign.clone());
                    }
                }
                decisions[if native { 2 } else { usize::from(fused) }] = kernel.decisions;
                artifact::json(&config.output.join("sources").join(format!("{case}-{repetition}-outputs.json")),
                    &collector.values.iter().map(|v| json!({"port":v.front_port_id.as_str(),"kind":v.value_kind.as_str(),"sequence":v.sequence,"bytes":&v.bytes[..]})).collect::<Vec<_>>())?;
                let first = collector.first.ok_or("first fused output missing")?;
                let last = collector.last.ok_or("last fused output missing")?;
                let row = json!({
                    "case":case,"phase":"finite-ordinary-fusion-pipeline","mode":mode,"repetition":repetition,"gears":gears,"values":ITEMS,
                    "source_sha256":artifact::digest(authored.as_bytes()),"checked_plot_id":prepared.plan.checked_plot_id.as_str(),
                    "ordered_output_sha256":digest,"full_host_call":metrics,"kernel_phase_wall":kernel_phase_wall,"cold_phases":prepared.phases,"planning_selection":prepared.selection_evidence,
                    "first_acknowledgement_from_call_start_ns":timing::ns(first.duration_since(started)),
                    "first_to_last_acknowledgement_ns":timing::ns(last.duration_since(first)),
                    "completion_window_useful_outputs_per_second":(ITEMS-1) as f64 / last.duration_since(first).as_secs_f64(),
                    "post_first_output_allocations":collector.allocation_window()?,
                    "kernel_decisions":kernel.decisions,"kernel_logical_invocations":kernel.logical_invocations,"physical_regions":prepared.plan.fragments[0].execution_fusions.len(),
                    "logical_gear_count":prepared.plan.fragments[0].placements.len(),"logical_internal_cord_count":prepared.plan.fragments[0].connections.len(),
                    "mandatory_output_sign_count":mandatory,"active_play":kernel.active_play,
                    "sign_scope":"all retained std report records; bounded transient history is not a full lifetime trace",
                    "sign_comparison":"all profiles: ordered remote/close/completion identities excluding sequence, same logical invocation count; canonical/native fused: exact retained records including sequence",
                    "value_capacity_before":kernel.value_allocation_capacity_before,"value_capacity_after":kernel.value_allocation_capacity_after,
                    "work_accounting":{"logical_evaluations":gears*ITEMS,"intermediate_materializations":if native {0} else {(gears-1)*ITEMS},"intermediate_canonical_bytes":if native {0} else {(gears-1)*ITEMS*8},
                        "native_word_transports":if native {(gears-1)*ITEMS} else {0},
                        "basis":"exact fixed fixture derivation from selected profile; logical Cord slots retained; native interior uses words, boundary codecs retained; not hardware traffic or a store counter", "internal_copy_counter":null},
                    "scope":"full call includes reservation, Back preparation, bounded kernel Play and report translation; completion window includes Host/collector work; std only",
                    "signs":kernel.kernel_sign.iter().map(|e|json!({"sequence":e.sequence,"node":e.node.0,"kind":format!("{:?}",e.kind),"port":e.port.map(|p|p.0),"request":e.request.map(|r|r.0)})).collect::<Vec<_>>(),
                });
                raw.row(&row)?;
                summaries.push(row);
            }
            if gears > 1 && (decisions[1] >= decisions[0] || decisions[2] != decisions[1]) {
                return Err("fusion did not reduce physical scheduler decisions".into());
            }
        }
    }
    Ok(())
}

fn required_lifecycle(events: &[conduit_kernel::KernelEvent]) -> Vec<conduit_kernel::KernelEvent> {
    use conduit_kernel::KernelEventKind::*;
    events
        .iter()
        .filter(|e| {
            matches!(
                e.kind,
                RemoteValueOffered
                    | RemoteValueAccepted
                    | RemoteValueDelivered
                    | RemoteOutputClosed
                    | RemoteInputAdmitted
                    | RemoteInputClosed
                    | InputClosed
                    | BackCompleted
            )
        })
        .map(|e| conduit_kernel::KernelEvent { sequence: 0, ..*e })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matched_profiles_retain_complete_artifacts_and_phase_allocations(
    ) -> Result<(), Box<dyn Error>> {
        let _measurement = crate::allocation::TEST_MEASUREMENT.lock().unwrap();
        let output = std::env::temp_dir().join(format!(
            "conduit-native-measurement-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        std::fs::create_dir(&output)?;
        std::fs::create_dir(output.join("sources"))?;
        let config = Config::parse(&[
            "--output".into(),
            output.display().to_string(),
            "--repetitions".into(),
            "3".into(),
        ])?;
        let mut raw = artifact::Raw::new(&output.join("raw.jsonl"))?;
        let mut rows = Vec::new();
        run(&config, &mut raw, &mut rows)?;
        raw.finish()?;
        assert_eq!(rows.len(), 24);
        assert_eq!(
            rows.iter().filter(|r| r["mode"] == "native-fused").count(),
            6
        );
        for row in rows {
            assert_eq!(
                row["kernel_phase_wall"]["allocator_intervals"]
                    .as_array()
                    .unwrap()
                    .len(),
                4
            );
            let stem = format!(
                "{}-{}",
                row["case"].as_str().unwrap(),
                row["repetition"].as_u64().unwrap()
            );
            let outputs: Value = serde_json::from_slice(&std::fs::read(
                output.join("sources").join(format!("{stem}-outputs.json")),
            )?)?;
            assert_eq!(outputs.as_array().unwrap().len(), ITEMS);
            assert!(row["signs"]
                .as_array()
                .unwrap()
                .iter()
                .all(|s| s.get("port").is_some() && s.get("request").is_some()));
        }
        drop(raw);
        std::fs::remove_dir_all(output)?;
        Ok(())
    }
}
