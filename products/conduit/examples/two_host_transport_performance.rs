//! Bounded actual two-Host Source→Plan→kernels→WebSocket API lifetime probe.
#[path = "two_host_transport_performance/allocation.rs"]
mod allocation;
#[path = "two_host_transport_performance/retention.rs"]
mod retention;
use serde_json::json;
use std::{fs, io::Write, path::PathBuf, process::Command};
#[global_allocator]
static ALLOCATOR: allocation::Allocator = allocation::Allocator;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = None;
    let mut repetitions = 3usize;
    let mut rotation = 0usize;
    let mut args = std::env::args().skip(1);
    while let Some(option) = args.next() {
        let value = args.next().ok_or("missing argument value")?;
        match option.as_str() {
            "--output" => output = Some(PathBuf::from(value)),
            "--repetitions" => repetitions = value.parse()?,
            "--rotation" => rotation = value.parse()?,
            _ => return Err(format!("unknown argument {option}").into()),
        }
    }
    if !(1..=20).contains(&repetitions) || rotation > 2 {
        return Err("capture bounds refused".into());
    }
    let output = output.ok_or("--output required")?;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let git = |args: &[&str]| -> Result<String, Box<dyn std::error::Error>> {
        let result = Command::new("git").args(args).current_dir(&root).output()?;
        if !result.status.success() {
            return Err("source custody probe failed".into());
        }
        Ok(String::from_utf8(result.stdout)?.trim().to_owned())
    };
    let source_sha = git(&["rev-parse", "HEAD"])?;
    if option_env!("CONDUIT_TWO_HOST_PERFORMANCE_SOURCE") != Some(source_sha.as_str())
        || !git(&["status", "--porcelain"])?.is_empty()
    {
        return Err("worker requires its exact sealed clean source; enter through xtask".into());
    }
    let mut trials = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("trials.jsonl"))?;
    let cases = [
        ("tiny", 8usize, 'x'),
        ("medium", 64, 'y'),
        ("large", 256, 'z'),
    ];
    let mut rows = 0;
    for repetition in 0..repetitions {
        for offset in 0..3 {
            let (case, length, character) = cases[(offset + repetition + rotation) % 3];
            let ((payload, authored, mut displayed), fixture) = allocation::observe(|| {
                let payload = character.to_string().repeat(length);
                let authored = format!("plot transport_{case} {{\n message: text/literal(\"{payload}\")\n show: face/text(\"Message\")\n message >> show\n}}\n");
                (payload, authored, Vec::with_capacity(length + 1))
            });
            let (execution, api) = allocation::observe(|| {
                conduit::execute_hosted_two_std_plot_with_evidence(&authored, &mut displayed)
            });
            let (execution, evidence) = execution?;
            if execution.result != payload
                || displayed != format!("{payload}\n").as_bytes()
                || execution.transferred_values != 1
            {
                return Err("two-Host exact output refused".into());
            }
            let source_evidence = retention::retain(&evidence.source)?;
            let sink_evidence = retention::retain(&evidence.sink)?;
            let receipt = json!({"source_document_id":execution.source_document_id.as_str(),"checked_plot_id":execution.checked_plot_id.as_str(),"expanded_plot_id":execution.expanded_plot_id.as_str(),"plan_id":execution.plan_id.as_str(),"source_fragment_id":execution.source_fragment_id.as_str(),"sink_fragment_id":execution.sink_fragment_id.as_str(),"source_active_play_id":execution.source_active_play_id.as_str(),"sink_active_play_id":execution.sink_active_play_id.as_str(),"line_id":execution.line_id.as_str(),"result":execution.result.as_str(),"transferred_values":execution.transferred_values});
            let phases = evidence
                .phases
                .iter()
                .map(|p| json!({"phase":p.phase,"elapsed_ns":p.elapsed_ns}))
                .collect::<Vec<_>>();
            let authored_retained = authored.clone();
            let (_, disposal) =
                allocation::observe(|| drop((execution, evidence, payload, authored, displayed)));
            let row = json!({"repetition":repetition,"case":case,"payload_bytes":length,"first_api_call_of_process":rows==0,"fresh_plan_and_line":true,"warm_line":false,"authored_source":authored_retained,"fixture":fixture,"api":api,"final_disposal":disposal,"api_phases":phases,"receipt":receipt,"source_kernel":source_evidence,"sink_kernel":sink_evidence});
            serde_json::to_writer(&mut trials, &row)?;
            trials.write_all(b"\n")?;
            trials.flush()?;
            rows += 1;
        }
    }
    if git(&["rev-parse", "HEAD"])? != source_sha || !git(&["status", "--porcelain"])?.is_empty() {
        return Err("source changed during capture".into());
    }
    let report = json!({"schema":"conduit.two-host-performance@1","source_commit":source_sha,"repetitions":repetitions,"rotation":rotation,"trials":rows,"trace":"trials.jsonl","proof_class":"hosted-loopback-two-logical-hosts","allocation_scope":"Successful Rust global allocation/reallocation/deallocation requests from all threads, including joined sink; request bytes, not peak live memory, OS/socket allocations or hardware traffic. Meter overhead is included.","timing_scope":"Fixture preparation, whole observed API and final disposal measured; API phase timing includes retained evidence copying. Correctness validation, JSON reconstruction/writing, process startup and metadata probes excluded. First API call marked; all subsequent calls still create fresh Plans/kernels/Lines. Fixed development Host/Boot identities and first-Play sequence; not persistent installed Hosts or warm transport.","missing_metrics":["isolated session encode/decode/framing and explicit copy attribution","warm established Line workload"],"timing_acceptance":"not established by worker; independent environment and custody acceptance required"});
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("report.json"))?;
    serde_json::to_writer_pretty(&mut file, &report)?;
    file.write_all(b"\n")?;
    println!("two-Host capture: {rows} actual loopback transfers; source {source_sha}");
    Ok(())
}
