//! Bounded overlapping streams through normal Plan/std Back/kernel execution.
use super::*;
use conduit_std_host::{ExternalForeStreamConfig, RunControl};
mod observation;
use observation::StreamCollector;
const STREAM_ITEMS: usize = 1024;
#[derive(Debug, Clone, Copy, serde::Serialize)]
struct Case {
    branches: usize,
    chain_gears: usize,
    queue: u16,
    batch: u16,
    payload_bytes: usize,
    text: bool,
    slow_sink_us: u64,
}
impl Case {
    fn id(self) -> String {
        format!(
            "stream-{}-branches-chain{}-q{}-batch{}-bytes{}-slow{}",
            self.branches,
            self.chain_gears,
            self.queue,
            self.batch,
            self.payload_bytes,
            self.slow_sink_us
        )
    }
    fn source(self) -> String {
        if let Some(additions) = 32_usize.checked_div(self.chain_gears) {
            let expression = crate::expression::addition_source(additions);
            return format!("plot baseline (\n >> input: U64...|\n output0: U64...| >>\n) {{\n input{} >> output0\n}}\n", format!(" >> ({expression})").repeat(self.chain_gears));
        }
        let ty = if self.text { "Text" } else { "U64" };
        let mut source = format!("plot baseline (\n >> input: {ty}...|\n");
        for branch in 0..self.branches {
            source += &format!(" output{branch}: {ty}...| >>\n");
        }
        source += ") {\n";
        let expression = if self.text {
            ".".to_string()
        } else {
            crate::expression::addition_source(32)
        };
        for branch in 0..self.branches {
            source += &format!(" input >> ({expression}) >> output{branch}\n");
        }
        source + "}\n"
    }
    fn input(self, index: usize) -> Vec<u8> {
        if self.text {
            format!("{index:08}{}", "x".repeat(self.payload_bytes - 8)).into_bytes()
        } else {
            (index as u64).to_le_bytes().to_vec()
        }
    }
    fn expected(self, index: usize) -> Vec<u8> {
        if self.text {
            self.input(index)
        } else {
            (index as u64 + 32).to_le_bytes().to_vec()
        }
    }
    fn acknowledge(self, branch: usize) {
        if branch == 0 && self.slow_sink_us != 0 {
            std::thread::sleep(std::time::Duration::from_micros(self.slow_sink_us));
        }
    }
}
fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for branches in [1, 4] {
        for queue in [1, 4] {
            for batch in [1, 8] {
                cases.push(Case {
                    branches,
                    chain_gears: 0,
                    queue,
                    batch,
                    payload_bytes: 8,
                    text: false,
                    slow_sink_us: 20,
                });
            }
        }
    }
    for branches in [1, 4] {
        cases.push(Case {
            branches,
            chain_gears: 0,
            queue: 1,
            batch: 8,
            payload_bytes: 8,
            text: false,
            slow_sink_us: 0,
        });
    }
    for payload_bytes in [32, 256] {
        cases.push(Case {
            branches: 4,
            chain_gears: 0,
            queue: 1,
            batch: 8,
            payload_bytes,
            text: true,
            slow_sink_us: 20,
        });
    }
    for chain_gears in [8, 32] {
        cases.push(Case {
            branches: 1,
            chain_gears,
            queue: 1,
            batch: 8,
            payload_bytes: 8,
            text: false,
            slow_sink_us: 0,
        });
    }
    cases
}
fn prepare_case(case: Case, boot: &str, count: usize) -> Result<Prepared, Box<dyn Error>> {
    let mut prepared = prepare_queue_mode(
        if case.chain_gears > 0 {
            case.chain_gears
        } else {
            case.branches
        },
        &case.source(),
        boot,
        BackProfile::Hosted,
        false,
        case.queue,
        (case.payload_bytes * usize::from(case.queue)) as u32,
    )?;
    prepared.inputs = (0..count)
        .map(|i| ExternalForeInput {
            front_port_id: "input".into(),
            track: ConnectionTrack::Payload,
            bytes: case.input(i),
        })
        .collect();
    Ok(prepared)
}
// Both native controls retain canonical payloads and the same synchronous slow
// acknowledgement. The library control uses an ordinary iterator composition.
// They omit typed Plan/Sign/Host transactions; that difference is reported.
fn native(
    case: Case,
    inputs: &[ExternalForeInput],
    library: bool,
) -> Result<Vec<(usize, u64, Vec<u8>)>, String> {
    let evaluate = |input: &ExternalForeInput| -> Result<Vec<u8>, String> {
        if case.text {
            return Ok(input.bytes.clone());
        }
        let mut value = std::hint::black_box(u64::from_le_bytes(
            input
                .bytes
                .as_slice()
                .try_into()
                .map_err(|_| "native U64 width")?,
        ));
        for _ in 0..32 {
            value = value.checked_add(1).ok_or("native overflow")?;
        }
        Ok(value.to_le_bytes().to_vec())
    };
    if library {
        inputs
            .iter()
            .enumerate()
            .flat_map(|(index, input)| (0..case.branches).map(move |branch| (index, input, branch)))
            .map(|(index, input, branch)| {
                let bytes = evaluate(input)?;
                case.acknowledge(branch);
                Ok((branch, index as u64, bytes))
            })
            .collect()
    } else {
        let mut out = Vec::with_capacity(inputs.len() * case.branches);
        for (index, input) in inputs.iter().enumerate() {
            let bytes = evaluate(input)?;
            for branch in 0..case.branches {
                case.acknowledge(branch);
                out.push((branch, index as u64, bytes.clone()));
            }
        }
        Ok(out)
    }
}
pub(super) fn run(
    config: &Config,
    raw: &mut artifact::Raw,
    summaries: &mut Vec<Value>,
) -> Result<(), Box<dyn Error>> {
    let identity = artifact::digest(config.output.to_string_lossy().as_bytes());
    for case in cases() {
        let id = case.id();
        artifact::new_file(&config.output.join("sources").join(format!("{id}.plot")))?
            .write_all(case.source().as_bytes())?;
        for repetition in 0..config.repetitions {
            let boot = format!("boot/runtime-stream/{identity}/{id}/{repetition}");
            let mut prepared = prepare_case(case, &boot, STREAM_ITEMS)?;
            let prefix = config
                .output
                .join("sources")
                .join(format!("{id}-{repetition}"));
            artifact::json(
                &prefix.with_file_name(format!("{id}-{repetition}-plan.json")),
                &prepared.plan,
            )?;
            artifact::json(
                &prefix.with_file_name(format!("{id}-{repetition}-host.json")),
                prepared.host.advertisement(),
            )?;
            let mut controls = Vec::new();
            // Rotate trial order across repetitions instead of always heating one mode first.
            let mut collector = StreamCollector::new(case, STREAM_ITEMS);
            let mut phases = phase_observation::PhaseObserver::default();
            let fragment = prepared.plan.fragments[0].clone();
            let mut kernel_report = None;
            let mut kernel_metrics = None;
            for mode in (0..3).map(|offset| (offset + usize::from(repetition)) % 3) {
                if mode == 0 {
                    let (result, metrics) = timing::measure(|| {
                        collector.begin();
                        phases.begin();
                        let result = prepared.host.run_external_plot_stream_controlled_to(
                            fragment.clone(),
                            &prepared.inputs,
                            &mut collector,
                            &mut std::io::sink(),
                            &mut phases,
                            &RunControl::default(),
                            ExternalForeStreamConfig {
                                admission_batch: case.batch,
                            },
                        );
                        phases.end();
                        result
                    });
                    kernel_report = Some(result?);
                    kernel_metrics = Some(metrics);
                } else {
                    let (result, metrics) =
                        timing::measure(|| native(case, &prepared.inputs, mode == 2));
                    let values = result?;
                    let digest = observation::validate_native(case, STREAM_ITEMS, &values)?;
                    controls.push(json!({"mode":if mode==1 {"native-fused-loop"} else {"native-library-pipeline"},"metrics":metrics,"ordered_output_sha256":digest}));
                }
            }
            let report = kernel_report.ok_or("stream was not measured")?;
            let metrics = kernel_metrics.ok_or("stream metrics absent")?;
            let kernel = report.kernel.as_ref().ok_or("stream kernel absent")?;
            if !matches!(
                report.observations.last().map(|o| &o.kind),
                Some(conduit_core::ObservationKind::PlanTerminal {
                    disposition: conduit_core::TerminalDisposition::Completed
                })
            ) {
                return Err("stream did not complete".into());
            }
            let observations = collector.report()?;
            if controls
                .iter()
                .any(|c| c["ordered_output_sha256"] != observations["ordered_output_sha256"])
            {
                return Err("stream/native digest differs".into());
            }
            let ingress = kernel
                .external_fore_stream
                .ok_or("stream admission observations absent")?;
            if usize::from(ingress.accepted) != STREAM_ITEMS
                || ingress.attempts != u32::from(ingress.accepted) + ingress.full
            {
                return Err("stream admission counts differ".into());
            }
            let output_signs = kernel
                .kernel_sign
                .iter()
                .filter(|e| e.kind == conduit_kernel::KernelEventKind::RemoteValueDelivered)
                .count();
            if output_signs != STREAM_ITEMS * case.branches
                || kernel.value_allocation_capacity_before != kernel.value_allocation_capacity_after
            {
                return Err("stream lost mandatory Signs or grew value storage".into());
            }
            let signs=kernel.kernel_sign.iter().map(|e|json!({"sequence":e.sequence,"node":e.node.0,"port":e.port.map(|p|p.0),"request":e.request.map(|r|r.0),"kind":format!("{:?}",e.kind)})).collect::<Vec<_>>();
            let phase_wall = phases.report()?;
            let row = json!({"case":id,"phase":"bounded-ordinary-stream","mode":"bounded-stream","repetition":repetition,"configuration":case,"input_items":STREAM_ITEMS,"useful_outputs":STREAM_ITEMS*case.branches,
                "source_sha256":artifact::digest(case.source().as_bytes()),"cold_phases":prepared.phases,"active_play":kernel.active_play,"checked_plot_id":prepared.plan.checked_plot_id,"full_host_call":metrics,"kernel_phase_wall":phase_wall,
                "observations":report.observations,"ingress":ingress,"output_observations":observations,"native_controls":controls,"kernel_decisions":kernel.decisions,"kernel_logical_invocations":kernel.logical_invocations,"signs":signs,
                "value_capacity_before":kernel.value_allocation_capacity_before,"value_capacity_after":kernel.value_allocation_capacity_after,
                "canonical_input_bytes":STREAM_ITEMS*case.payload_bytes,"canonical_output_bytes":STREAM_ITEMS*case.branches*case.payload_bytes,"internal_copy_counter":Value::Null,
                "scope":"finite preloaded overlapping stream, no drops allowed; slow sink is synchronous Host acknowledgement sleep on output0, not an asynchronous task; per-delivery clock observation included; global inter-branch order may differ, per-branch order and bytes must match; native controls include same sink dwell but omit typed Plan/Sign transactions; no hardware counters or zero-copy claim"});
            raw.row(&row)?;
            summaries.push(json!({"case":id,"mode":"bounded-stream","repetition":repetition,"full_host_call":metrics,"kernel_phase_wall":phase_wall}));
            eprintln!("validated {id} repetition {}", repetition + 1);
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests;
