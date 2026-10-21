//! Release empirical FARGAN worker; launched by the retained xtask entrance.
#![allow(dead_code)]
#[allow(unexpected_cfgs)]
#[path = "../../../proof/performance/runtime/src/allocation.rs"]
mod allocation;
#[allow(unexpected_cfgs)]
#[path = "../../../proof/performance/runtime/src/artifact.rs"]
mod artifact;
#[path = "../tests/fargan_signal_graph/conditioning.rs"]
mod conditioning;
#[path = "../tests/fargan_signal_graph/frame.rs"]
mod frame;
#[path = "../tests/fargan_signal_graph/pinned.rs"]
mod pinned;
#[path = "../tests/fargan_signal_graph/plan.rs"]
mod plan;
#[path = "../tests/fargan_signal_graph/runtime.rs"]
mod runtime;
#[path = "../tests/fargan_signal_graph/shared.rs"]
mod shared;
#[path = "../tests/fargan_signal_graph/state.rs"]
mod state;
#[path = "../../../proof/performance/runtime/src/timing.rs"]
mod timing;
#[global_allocator]
static ALLOCATOR: allocation::Allocator = allocation::Allocator;
const STACK_BYTES: usize = 32 * 1024 * 1024;
#[path = "fargan_performance/prepared.rs"]
mod prepared_performance;
#[path = "fargan_performance/sign_text.rs"]
mod sign_text;
use serde_json::json;
use std::{cell::RefCell, path::PathBuf};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    if cfg!(debug_assertions) {
        return Err("release measurements required".into());
    }
    let args: Vec<_> = std::env::args().skip(1).collect();
    let (fixture, output, repetitions, owners) = match args.as_slice() {
        [fixture, output, repetitions] => (fixture, output, repetitions, "fresh"),
        [fixture, output, repetitions, owners] => (fixture, output, repetitions, owners.as_str()),
        _ => return Err("internal worker expects fixture/output/repetitions/[owners]".into()),
    };
    if !matches!(
        owners,
        "fresh"
            | "prepared"
            | "prepared-compact-local"
            | "prepared-matrix-local"
            | "prepared-owned-matrix-local"
            | "prepared-matrix-source-cache"
    ) {
        return Err("unknown owner profile".into());
    }
    let repetitions: u32 = repetitions.parse()?;
    assert!((2..=20).contains(&repetitions));
    let output = PathBuf::from(output);
    let fixture = PathBuf::from(fixture);
    let mut metadata = artifact::metadata()?;
    let worker = std::env::current_exe()?;
    metadata["worker_sha256"] = json!(artifact::digest(&std::fs::read(&worker)?));
    metadata["worker_path"] = json!(worker);
    metadata["owners"] = json!(owners);
    artifact::json(&output.join("metadata.json"), &metadata)?;
    let mut raw = artifact::Raw::new(&output.join("raw.jsonl"))?;
    raw.row(&json!({"phase":"timer-control", "samples_ns":timing::timer_control(2000),"subtracted":false}))?;
    let (schema, check) = timing::measure(frame::schema);
    raw.row(&json!({"phase":"cold-source-schema","metrics":check}))?;
    std::fs::write(output.join("source.conduit"), plan::source())?;
    let (resources, model_preparation) = timing::measure(|| frame::resources(&fixture, &schema));
    raw.row(&json!({"phase":"cold-model-resource-admission","metrics":model_preparation}))?;
    let oracle = frame::oracle(&fixture);
    artifact::json(
        &output.join("model-profile.json"),
        &json!({
            "upstream_sha":"503d81b138d76621aae4b12786e90de48aa8db3a",
            "generated_source_sha256":"68ba6e8731007f7184be862d7d1afe4cef096a7c9bcbfa115962340f725e004c",
            "f32_bundle_sha256":"d35f0510da476716183a33caafe45cc095e48d496f06de2db7655e780a385e47",
            "full_frame_oracle_sha256":"9c676f6c76aec71281ea097fe0c477c5a3b77f5022e639a0d82dbb4a5437fcc3",
            "precision_profile":"number/ieee754-f32-libm-reference@1","parameters_published":false
        }),
    )?;
    if owners != "fresh" {
        prepared_performance::run(
            &output,
            repetitions,
            &schema,
            &resources,
            &oracle,
            &mut raw,
            prepared_performance::Profile::from_name(owners)?,
        )?;
        raw.finish()?;
        return Ok(());
    }
    let cases = &oracle.cases;
    let mut walls = Vec::new();
    for repetition in 0..repetitions {
        let (plan, cold) =
            timing::measure(|| plan::prepare_entry_plan(&schema, &plan::source(), frame::ENTRY));
        raw.row(&json!({"phase":"cold-check-expand-plan","repetition":repetition,"metrics":cold}))?;
        artifact::json(
            &output.join(format!("plan-repetition-{repetition}.json")),
            &plan,
        )?;
        let plan_sha = artifact::digest(&serde_json::to_vec(&plan)?);
        let mut state = oracle.state.clone();
        let mut history = oracle.history;
        let mut previous = oracle.period;
        let mut history_error = 0f32;
        let mut pcm_error = 0f32;
        let mut state_error = 0f32;
        for (index, case) in cases.iter().enumerate() {
            let input = frame::inputs(
                &schema,
                &case.features,
                &history,
                case.period,
                previous,
                &state,
            );
            let metrics = RefCell::new(None);
            let cpu = RefCell::new(None);
            let result = runtime::run_entry_plan_observed(
                plan.clone(),
                &resources,
                input,
                runtime::ExecutionMode::Normal,
                || {
                    *cpu.borrow_mut() = timing::cpu();
                    allocation::start();
                },
                || {
                    *metrics.borrow_mut() = Some((
                        allocation::finish(),
                        cpu.borrow().zip(timing::cpu()).map(|(a, b)| b - a),
                    ));
                },
            )
            .ok_or("no complete FARGAN result")?;
            let pcm = frame::pcm(&result);
            history = state::floats(&result.outputs["next_history"])
                .try_into()
                .unwrap();
            for (a, b) in history.iter().zip(case.history) {
                history_error = history_error.max((a - b).abs());
            }
            previous = case.period;
            let next = state::State::from_result(&result.value);
            for (a, b) in pcm.iter().zip(&case.pcm) {
                pcm_error = pcm_error.max((a - b).abs());
            }
            for (a, b) in next.flattened().iter().zip(case.next.flattened()) {
                state_error = state_error.max((a - b).abs());
            }
            let sign_text = sign_text::render(&result.signs);
            let sign_sha = artifact::digest(sign_text.as_bytes());
            if repetition == 0 && index == 0 {
                std::fs::write(output.join("signs-first-frame.txt"), &sign_text)?;
            }
            let work = result.work;
            let wall = timing::ns(result.execution);
            walls.push(wall);
            let (counts, process_cpu_ns) = metrics.into_inner().unwrap();
            assert_eq!(
                counts.allocations + counts.reallocations,
                0,
                "Play must retain prepared storage"
            );
            raw.row(&json!({"phase":"fargan-frame-play","repetition":repetition,"frame":index,
                "plan_sha256":plan_sha,"nodes":result.nodes,"cords":result.cords,"useful_pcm_samples":160,
                "process_cpu_ns":process_cpu_ns,"owner_preparation_ns":timing::ns(result.preparation),"wall_ns":wall,
                "step_wall_ns":timing::ns(work.kernel_steps),"numeric_back_wall_ns":timing::ns(work.numeric),
                "step_remainder_ns":timing::ns(work.kernel_steps.saturating_sub(work.numeric)),
                "expression_host_wall_ns":timing::ns(work.expressions),"steps":work.steps,
                "numeric_steps":work.numeric_steps,"expression_calls":work.expression_calls,
                "signs":work.signs,"signs_sha256":sign_sha,"structural_drain":true,"pending_host_calls_at_end":0,"scheduler_inline_bytes":work.scheduler_bytes,"allocator":counts,
                "output_sha256":artifact::digest(&serde_json::to_vec(&result.outputs.iter().map(|(n,v)|(n,v.canonical_bytes().unwrap())).collect::<Vec<_>>())?),
                "pcm_max_abs_error":pcm_error,"active_state_max_abs_error":state_error,"conditioning_history_max_abs_error":history_error}))?;
            state = next;
        }
        assert!(
            pcm_error < 0.002 && state_error < 0.05 && history_error < 0.001,
            "pinned oracle differential"
        );
        eprintln!(
            "validated FARGAN repetition {}: PCM error={pcm_error}, state error={state_error}",
            repetition + 1
        );
    }
    raw.finish()?;
    artifact::json(
        &output.join("summary.json"),
        &json!({
            "schema":"conduit.fargan-runtime-performance@1","worker_thread_stack_bytes":STACK_BYTES,"sign_digest_schema":"rust-debug-kernel-events@1","wall_distribution":timing::distribution(&walls),
            "repetitions":repetitions,"frames_per_repetition":24,"pcm_samples_per_frame":160,
            "source_sha256":artifact::digest(plan::source().as_bytes()),
            "scope":"Full float conditioner and four recurrent subframes in one authored frame topology, with private pinned tensors and own accumulated conditioning/history/state. Reprepared Value invocations; no streaming, realtime, or intelligibility claim.",
            "timing_boundary":"Step remainder includes kernel dispatch, transport, Sign, Back commit, and observer overhead. Numeric includes exact codecs. Host expressions measured separately. Timers are not subtracted."
        }),
    )?;
    Ok(())
}
fn main() {
    let result = std::thread::Builder::new()
        .stack_size(STACK_BYTES)
        .spawn(|| run().map_err(|e| e.to_string()))
        .unwrap()
        .join()
        .unwrap();
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
fn on_large_stack(work: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(STACK_BYTES)
        .spawn(work)
        .unwrap()
        .join()
        .unwrap();
}
