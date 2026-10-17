//! Release-only empirical baseline worker launched through cargo xtask.
mod allocation;
mod artifact;
mod config;
mod expression;
#[cfg(feature = "hosted-pipeline")]
mod pipeline;
mod pool;
mod repetition;
mod timing;
mod typed_cost;
#[global_allocator]
static ALLOCATOR: allocation::Allocator = allocation::Allocator;
use expression::{Domain, Shape, Workload};
use serde_json::json;
use std::{error::Error, fs, io::Write};

fn run() -> Result<(), Box<dyn Error>> {
    if cfg!(debug_assertions) {
        return Err("runtime-performance measurements require the xtask release worker".into());
    }
    let config = config::Config::parse(&std::env::args().skip(1).collect::<Vec<_>>())?;
    if config.hosted_pipeline && !cfg!(feature = "hosted-pipeline") {
        return Err("hosted pipeline requires its admitted worker feature".into());
    }
    let metadata = artifact::metadata()?;
    artifact::json(&config.output.join("metadata.json"), &metadata)?;
    fs::create_dir(config.output.join("sources"))?;
    let mut raw = artifact::Raw::new(&config.output.join("raw.jsonl"))?;
    let controls = timing::timer_control(config.samples);
    raw.row(&json!({"case":"timer-control", "samples_ns":controls, "distribution":timing::distribution(&controls), "subtracted":false}))?;
    let mut summaries = Vec::new();
    for domain in [Domain::U64, Domain::Scalar] {
        for shape in [
            Shape::Add(1),
            Shape::Add(3),
            Shape::Add(8),
            Shape::Add(32),
            Shape::Conditional,
            Shape::Comparison,
        ] {
            let workload = Workload { domain, shape };
            let id = workload.id();
            let source = workload.source();
            artifact::new_file(&config.output.join("sources").join(format!("{id}.plot")))?
                .write_all(source.as_bytes())?;
            for repetition in 0..config.repetitions {
                let (program, cold) = expression::cold(workload)?;
                raw.row(&json!({"case":id,"phase":"cold-expression","repetition":repetition,"source_sha256":artifact::digest(source.as_bytes()),"program_sha256":artifact::digest(&program.canonical_bytes().map_err(|e|format!("codec: {e:?}"))?),"phases":cold}))?;
                let order = if repetition % 2 == 0 {
                    [false, true]
                } else {
                    [true, false]
                };
                let mut checksum = None;
                for prepared in order {
                    let warm = expression::warm(workload, &program, &config, prepared)?;
                    if checksum.as_ref().is_some_and(|prior| {
                        prior != &(warm.validation_sum.clone(), warm.validation_sha256.clone())
                    }) {
                        return Err(format!("{id}: native/prepared outputs differ").into());
                    }
                    checksum = Some((warm.validation_sum.clone(), warm.validation_sha256.clone()));
                    summaries.push(json!({"case":id,"mode":warm.mode,"repetition":repetition,"values":warm.values,"operations":warm.primitive_operations,"bulk_wall_ns":warm.metrics.wall_ns,"bulk_cpu_ns":warm.metrics.process_cpu_ns,"allocator":warm.metrics.allocator,"latency_distribution":warm.latency_distribution}));
                    raw.row(&json!({"case":id,"phase":"warm-expression","repetition":repetition,"source_sha256":artifact::digest(source.as_bytes()),"run":warm}))?;
                }
                eprintln!("validated {id} repetition {}", repetition + 1);
            }
        }
    }
    for realizations in [1, 8, 32] {
        for resources in [1, 4] {
            let fixture = pool::Fixture::new(realizations, resources);
            let id = format!("pool-selection-{realizations}-realizations-{resources}-resources");
            artifact::json(
                &config.output.join("sources").join(format!("{id}.json")),
                &fixture.artifact(),
            )?;
            for repetition in 0..config.repetitions {
                let run = pool::run(&fixture, &config)?;
                summaries.push(json!({"case":id,"mode":"production-selection-isolation","repetition":repetition,"decisions":run.decisions,"selection_wall_ns":run.summed_selection_wall_ns,"whole_trial_loop":run.whole_trial_loop,"latency_distribution":run.latency_distribution,"examined_realizations":run.examined_realizations,"lifecycle_signs":run.lifecycle_signs}));
                raw.row(
                    &json!({"case":id,"phase":"pool-selection","repetition":repetition,"run":run}),
                )?;
                eprintln!("validated {id} repetition {}", repetition + 1);
            }
        }
    }
    for candidates in [1, 8, 32] {
        let fixture = typed_cost::Fixture::new(candidates)?;
        let id = format!("typed-cost-selection-{candidates}-hosts");
        artifact::json(
            &config.output.join("sources").join(format!("{id}.json")),
            &fixture.artifact(),
        )?;
        for repetition in 0..config.repetitions {
            let run = typed_cost::run(&fixture, &config)?;
            summaries.push(json!({"case":id,"mode":"production-typed-cost-selection-isolation","repetition":repetition,"decisions":run.decisions,"selection_wall_ns":run.summed_selection_wall_ns,"latency_distribution":run.latency_distribution,"arithmetic_work_spent":run.arithmetic_work_spent}));
            raw.row(&json!({"case":id,"phase":"typed-cost-selection","repetition":repetition,"run":run}))?;
            eprintln!("validated {id} repetition {}", repetition + 1);
        }
    }
    #[cfg(feature = "hosted-pipeline")]
    if config.hosted_pipeline {
        pipeline::run(&config, &mut raw, &mut summaries)?;
        for candidates in [1, 8, 32] {
            let fixture = typed_cost::Fixture::with_std_inventory(candidates)?;
            let id = format!("typed-cost-selection-{candidates}-hosts-std-owner-inventory");
            artifact::json(
                &config.output.join("sources").join(format!("{id}.json")),
                &fixture.artifact(),
            )?;
            for repetition in 0..config.repetitions {
                let run = typed_cost::run(&fixture, &config)?;
                summaries.push(json!({"case":id,"mode":"production-typed-cost-selection-isolation","repetition":repetition,
                    "inventory":run.inventory,"offers_per_host":run.offers_per_host,"decisions":run.decisions,
                    "selection_wall_ns":run.summed_selection_wall_ns,"latency_distribution":run.latency_distribution,
                    "arithmetic_work_spent":run.arithmetic_work_spent}));
                raw.row(&json!({"case":id,"phase":"typed-cost-selection","repetition":repetition,"run":run}))?;
                eprintln!("validated {id} repetition {}", repetition + 1);
            }
        }
    }
    raw.finish()?;
    artifact::json(
        &config.output.join("report.json"),
        &json!({
            "schema":"conduit.runtime-performance-report@1",
            "status":if config.hosted_pipeline { "measured-isolation-and-finite-hosted-baseline" } else { "measured-isolation-baseline" }, "regression_gate":"report-only",
            "config":config,"metadata":"metadata.json","raw":"raw.jsonl","raw_records":raw.records,"runs":summaries,
            "process_resident_high_water":artifact::resident_high_water(),
            "repeated_batches":repetition::summarize(&summaries),
            "confidence":"descriptive sample standard deviation across repeated batches; raw repeated batches and per-event percentiles; no certified independent-run confidence interval or speedup claim; repeatability gate remains open",
            "scope":if config.hosted_pipeline {
                "production evaluator/typed-cost/pool-selection isolation plus 64-item ordinary checked Plot/Plan/std Back/kernel 1/8/32-Gear trials; separate cold Back/Play phases, continuous streams, DSP/pressure/fairness and browser matrix remain required by #5395"
            } else {
                "production prepared expression evaluator and production shared-pool and typed-cost selection isolation; full ordinary Plan/Play/DSP/pressure/browser matrix remains required by #5395"
            }
        }),
    )?;
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("runtime-performance refused: {error}");
        std::process::exit(1);
    }
}
