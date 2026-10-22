//! Release diagnostic launched by cargo xtask; not an installed Body migration.
#[allow(dead_code, unexpected_cfgs)]
#[path = "../../../proof/performance/runtime/src/allocation.rs"]
mod allocation;
#[path = "resource_replacement_cost/fixture.rs"]
mod fixture;
#[path = "resource_replacement_cost/measurement.rs"]
mod measurement;
#[allow(dead_code)]
#[path = "../../../proof/performance/runtime/src/timing.rs"]
mod timing;
#[global_allocator]
static ALLOCATOR: allocation::Allocator = allocation::Allocator;
use std::{error::Error, fs, path::PathBuf};
fn main() -> Result<(), Box<dyn Error>> {
    if cfg!(debug_assertions) {
        return Err("release resource-cost worker required".into());
    }
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    let [output, repetitions] = arguments.as_slice() else {
        return Err("output and repetitions required".into());
    };
    let repetitions: u16 = repetitions.parse()?;
    if !(3..=20).contains(&repetitions) {
        return Err("repetitions must be 3..20".into());
    }
    let output = PathBuf::from(output);
    let executable = std::env::current_exe()?;
    use sha2::{Digest, Sha256};
    fs::write(
        output.join("worker.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "binary_sha256":format!("{:x}",Sha256::digest(fs::read(&executable)?)),
            "profile":"release","debug_assertions":cfg!(debug_assertions),
            "compiler":String::from_utf8(std::process::Command::new("rustc").arg("-vV").output()?.stdout)?,
            "cpu":fs::read_to_string("/proc/cpuinfo").ok().and_then(|s|s.lines().find(|l|l.starts_with("model name")).map(str::to_owned)),
            "scope":"Linux actual software resource provider and resource-owner ledger; no physical device or installed Body migration"
        }))?,
    )?;
    let (cases, planning) = timing::measure(fixture::Cases::new);
    let cases = cases?;
    fs::write(
        output.join("plans.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"source":fixture::SOURCE,"fixture_source_and_planning":planning,"host":cases.host,"previous":cases.previous,"candidate":cases.candidate}),
        )?,
    )?;
    let rows = measurement::run(&cases, repetitions)?;
    fs::write(
        output.join("measurements.json"),
        serde_json::to_vec_pretty(&rows)?,
    )?;
    println!(
        "retained {} actual provider cases",
        rows["trials"].as_array().ok_or("missing trials")?.len()
    );
    Ok(())
}
