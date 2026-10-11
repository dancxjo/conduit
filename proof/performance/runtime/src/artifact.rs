//! Immutable retained raw rows, exact source custody, and bounded run metadata.
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::Path,
    process::Command,
};
pub fn text(command: &str, arguments: &[&str]) -> Option<String> {
    let output = Command::new(command).args(arguments).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn new_file(path: &Path) -> std::io::Result<File> {
    OpenOptions::new().write(true).create_new(true).open(path)
}
pub fn json(path: &Path, value: &impl serde::Serialize) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = new_file(path)?;
    serde_json::to_writer_pretty(&mut file, value)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}
pub struct Raw {
    file: File,
    pub records: usize,
}
impl Raw {
    pub fn new(path: &Path) -> std::io::Result<Self> {
        Ok(Self {
            file: new_file(path)?,
            records: 0,
        })
    }
    pub fn row(&mut self, value: &impl serde::Serialize) -> Result<(), Box<dyn std::error::Error>> {
        serde_json::to_writer(&mut self.file, value)?;
        self.file.write_all(b"\n")?;
        self.file.flush()?;
        self.records += 1;
        Ok(())
    }
    pub fn finish(&mut self) -> std::io::Result<()> {
        self.file.sync_all()
    }
}
pub fn metadata() -> Result<serde_json::Value, String> {
    let sha = text("git", &["rev-parse", "HEAD"]).ok_or("git source identity unavailable")?;
    let status = text("git", &["status", "--porcelain"]).ok_or("source status unavailable")?;
    if !status.is_empty() {
        return Err(
            "freeze and commit the complete worktree before retaining a performance result".into(),
        );
    }
    let cpu_info = fs::read_to_string("/proc/cpuinfo").ok();
    let cpu = cpu_info.map(|s| {
        s.lines()
            .filter(|l| {
                [
                    "model name",
                    "cpu MHz",
                    "cpu cores",
                    "siblings",
                    "physical id",
                    "processor",
                ]
                .iter()
                .any(|key| l.starts_with(key))
            })
            .collect::<Vec<_>>()
            .join("\n")
    });
    let status = fs::read_to_string("/proc/self/status").ok();
    let affinity = status
        .as_ref()
        .and_then(|s| {
            s.lines()
                .find(|line| line.starts_with("Cpus_allowed_list:"))
        })
        .map(str::to_owned);
    Ok(serde_json::json!({
        "schema":"conduit.runtime-performance@1", "git_sha":sha,
        "source_status":"clean", "os":text("uname", &["-srvm"]),
        "compiler":text("rustc", &["-vV"]), "cargo":text("cargo", &["-V"]),
        "cpu":cpu, "affinity":affinity, "profile":"release", "debug_assertions":cfg!(debug_assertions),
        "rustflags":std::env::var("RUSTFLAGS").ok(),
        "release_overrides": {"opt_level":std::env::var("CARGO_PROFILE_RELEASE_OPT_LEVEL").ok(),"lto":std::env::var("CARGO_PROFILE_RELEASE_LTO").ok(),"codegen_units":std::env::var("CARGO_PROFILE_RELEASE_CODEGEN_UNITS").ok()},
        "binary_bytes": std::env::current_exe().ok().and_then(|p|fs::metadata(p).ok()).map(|m|m.len()),
        "cpu_time_scope":"Linux process CPU for each bulk phase; per-selection isolation uses wall clocks; unsupported target CPU time is null",
        "allocator_scope":"successful System allocator calls; atomic counters only on allocation/deallocation; heap peak includes retained harness data",
        "measurement_limitations":["no CPU affinity change", "background load is not controlled; independently rerun on an idle host", "no hardware cycles/instructions/branch-miss counters", "internal copy accounting not yet instrumented", "expression-only native uses registers; prepared uses canonical 8-byte inputs and canonical outputs; finite hosted native uses canonical input/output frames", "isolated selection trials do not model steady mixed-workload fairness or full Plot dispatch"],
        "required_follow_on_gates":["fully separated cold std Back preparation, Play, Sign and teardown timing beyond optional combined finite Host calls", "continuous 1/8/32 real Gear scaling beyond optional 64-item finite hosted trials", "streaming/pressure/fan-out/payload/batch/fairness", "DSP representative", "browser/WASM after hosted repeatability", "review independent-run uncertainty and regression thresholds"]
    }))
}
pub fn resident_high_water() -> Option<String> {
    fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find(|line| line.starts_with("VmHWM:"))
        .map(str::to_owned)
}
