//! Source-bound release observation through the ordinary reluctant execution owner.
#[path = "reluctant_replan/clock.rs"]
mod clock;
#[path = "reluctant_replan/evidence.rs"]
mod evidence;
#[path = "reluctant_replan/experiment.rs"]
mod experiment;
#[path = "../tests/replan_common/expression.rs"]
#[allow(dead_code)]
mod fixture;
use clock::*;
use serde_json::json;
use std::{fs, path::PathBuf, process::Command};
fn git(args: &[&str]) -> Result<String, Error> {
    let r = Command::new("git").args(args).output()?;
    if !r.status.success() {
        return Err("git custody probe failed".into());
    }
    Ok(String::from_utf8(r.stdout)?.trim().into())
}
fn main() -> Result<(), Error> {
    if cfg!(debug_assertions) || !cfg!(target_os = "linux") {
        return Err("requires supported Linux release entrance".into());
    }
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 1 {
        return Err("requires fresh output directory from xtask".into());
    }
    let output = PathBuf::from(&args[0]);
    let source = git(&["rev-parse", "HEAD"])?;
    if !git(&["status", "--porcelain"])?.is_empty() {
        return Err("commit clean source before observations".into());
    }
    let mut log = evidence::Log::new(&output.join("raw.jsonl"))?;
    let id = format!("{}/{}", std::process::id(), tick()?);
    let result = experiment::run(
        &mut log,
        format!("host/reluctant/{id}").into(),
        format!("boot/reluctant/{id}").into(),
    );
    let final_source = git(&["rev-parse", "HEAD"])?;
    let clean = git(&["status", "--porcelain"])?.is_empty();
    let status = json!({"schema":"conduit.reluctant-replan-worker@1","source":source,"final_source":final_source,"source_custody":clean&&source==final_source,"success":result.is_ok(),"error":result.as_ref().err().map(|e|e.to_string()),"rustc":String::from_utf8_lossy(&Command::new("rustc").arg("-vV").output()?.stdout),"physical_os_boot":fs::read_to_string("/proc/sys/kernel/random/boot_id").ok(),"cpu_model":fs::read_to_string("/proc/cpuinfo").ok().and_then(|s|s.lines().find(|l|l.starts_with("model name")).map(str::to_owned)),"arch":std::env::consts::ARCH,"os":std::env::consts::OS});
    fs::write(
        output.join("worker-status.json"),
        serde_json::to_vec_pretty(&status)?,
    )?;
    result?;
    if !clean || source != final_source {
        return Err("source custody changed".into());
    }
    Ok(())
}
