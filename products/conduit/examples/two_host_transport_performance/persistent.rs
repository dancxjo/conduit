//! One admitted, bounded Tick Flow and established Line per measured trial.
use super::{allocation, retention};
use serde_json::json;
use std::{fs, io::Write, path::Path};

pub fn capture(
    output: &Path,
    repetitions: usize,
    values: u16,
    source: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut trials = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("trials.jsonl"))?;
    for repetition in 0..repetitions {
        let (mut displayed, fixture) = allocation::observe(|| Vec::with_capacity(256));
        let (result, api) = allocation::observe(|| {
            conduit::execute_hosted_two_std_tick_flow_with_evidence(values, &mut displayed)
        });
        // Keep failures visible; a failed trial cannot become an excluded fast sample.
        let (execution, evidence) = match result {
            Ok(result) => result,
            Err(error) => {
                serde_json::to_writer(
                    &mut trials,
                    &json!({"repetition":repetition,"success":false,"error":error,"api":api,"displayed":displayed}),
                )?;
                trials.write_all(b"\n")?;
                trials.flush()?;
                return Err(error.into());
            }
        };
        let expected: String = (0..values).map(|n| format!("{n}\n")).collect();
        let validation = (|| -> Result<_, String> {
            if displayed != expected.as_bytes()
                || execution.result != (values - 1).to_string()
                || execution.transferred_values != u32::from(values)
            {
                return Err("persistent exact output refused".into());
            }
            let line = evidence.line.as_ref().ok_or("Line timings missing")?;
            if line.values != u32::from(values) {
                return Err("Line timing count differs".into());
            }
            Ok((
                retention::retain(&evidence.source)?,
                retention::retain(&evidence.sink)?,
            ))
        })();
        let (source_kernel, sink_kernel) = match validation {
            Ok(result) => result,
            Err(error) => {
                serde_json::to_writer(
                    &mut trials,
                    &json!({"repetition":repetition,"success":false,"error":error,"api":api,"displayed":displayed}),
                )?;
                trials.write_all(b"\n")?;
                trials.flush()?;
                return Err(error.into());
            }
        };
        let receipt = json!({"source_document_id":execution.source_document_id.as_str(),"checked_plot_id":execution.checked_plot_id.as_str(),"expanded_plot_id":execution.expanded_plot_id.as_str(),"plan_id":execution.plan_id.as_str(),"source_fragment_id":execution.source_fragment_id.as_str(),"sink_fragment_id":execution.sink_fragment_id.as_str(),"source_active_play_id":execution.source_active_play_id.as_str(),"sink_active_play_id":execution.sink_active_play_id.as_str(),"line_id":execution.line_id.as_str(),"result":execution.result,"transferred_values":execution.transferred_values});
        let line = serde_json::to_value(&evidence.line)?;
        let phases = evidence
            .phases
            .iter()
            .map(|p| json!({"phase":p.phase,"elapsed_ns":p.elapsed_ns}))
            .collect::<Vec<_>>();
        let exact_output = displayed.clone();
        let (_, disposal) = allocation::observe(|| drop((execution, evidence, displayed)));
        let row = json!({"repetition":repetition,"success":true,"values":values,"value_payload_bytes":8,"first_api_call_of_process":repetition==0,"fresh_plan_and_line":true,"persistent_line_within_trial":true,"fixture":fixture,"api":api,"api_phases":phases,"line_intervals":line,"final_disposal":disposal,"receipt":receipt,"exact_output":exact_output,"source_kernel":source_kernel,"sink_kernel":sink_kernel});
        serde_json::to_writer(&mut trials, &row)?;
        trials.write_all(b"\n")?;
        trials.flush()?;
    }
    let report = json!({"schema":"conduit.two-host-persistent-performance@1","source_commit":source,"repetitions":repetitions,"values_per_line":values,"trials":repetitions,"trace":"trials.jsonl","proof_class":"hosted-loopback-two-logical-hosts","allocation_scope":"Successful Rust global allocation/reallocation/deallocation requests across both joined owner threads for the whole API. Request bytes, not peak live memory or OS allocations.","timing_scope":"Each trial creates one fresh Plan and pair of kernels. All Values share one admitted Line/session. Source monotonic wall intervals distinguish connection/session, first Value, remaining Values, shutdown/retained evidence. Whole API includes preparation and evidence retention; final caller-held disposal is separate. Validation and JSON reconstruction are outside measurement.","missing_metrics":["per-stage allocation and copy attribution","CPU time","peak live memory","OS and hardware traffic"],"timing_acceptance":"local observation only; no calibrated throughput claim"});
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("report.json"))?;
    serde_json::to_writer_pretty(&mut file, &report)?;
    file.write_all(b"\n")?;
    println!("persistent two-Host capture: {repetitions} Lines, {values} Values per Line; source {source}");
    Ok(())
}
