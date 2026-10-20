//! Source-bound process observation around the ordinary graphical journey.
mod breakdown;
use super::{image, profile::Paths, report::git_head, ConduitosArch, ConduitosError};
use crate::cli::GlobalOpts;
use serde_json::json;
use std::{fs, path::Path, process::Command};

#[derive(clap::Args, Debug)]
pub struct NativeLatencyArgs {
    /// Fresh QEMU processes over one exact built image. No retry on failure.
    #[arg(long, default_value_t = 3, value_parser = clap::value_parser!(u8).range(1..=8))]
    pub trials: u8,
}

pub(super) fn execute(args: &NativeLatencyArgs, opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(refusal(
            "native latency observation needs a real image and guest",
        ));
    }
    if !(1..=8).contains(&args.trials) {
        return Err(refusal(
            "native observation admits one through eight trials",
        ));
    }
    let paths = Paths::new(ConduitosArch::X86_64)?;
    require_clean_source(&paths.root)?;
    let source = git_head(&paths.root)?;
    let rustc = tool_version("rustc", &["--version", "--verbose"])?;
    let qemu = tool_version("qemu-system-x86_64", &["--version"])?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| refusal("artifact clock unavailable"))?
        .as_nanos();
    let family = paths
        .target
        .join("native-latency")
        .join(format!("{}-{stamp}", std::process::id()));
    fs::create_dir_all(&family).map_err(io_error)?;
    let started = std::time::Instant::now();
    let mut trials = Vec::with_capacity(usize::from(args.trials));
    let mut context = json!({"schema":"conduit.conduitos/native-process-timing@1",
        "status":"running","proof_class":"freestanding-emulator","source_commit":source,
        "rustc":rustc,"qemu":qemu,"clock":"std::time::Instant","units":"nanoseconds",
        "admitted_trials":args.trials,
        "protocol":"Build one image, then run the same bounded journey in fresh QEMU processes without retries. First process and later processes are separate observations; compiler and host OS caches are uncontrolled. Preparation includes supported image build. Journey includes emulator startup, inputs, receipt polling, frame capture and teardown. Total wall time excludes xtask compilation and final receipt serialization. Preserve failed attempts; no cold-machine, calibrated speedup, or physical latency claim."
    });
    write(&family.join("manifest.json"), &context)?;
    let result = (|| {
        let built = image::execute_native_latency(opts)?;
        context["image_sha256"] = built.iso_sha256.clone().into();
        context["build_and_image_ns"] = json!(started.elapsed().as_nanos());
        for trial in 0..args.trials {
            let output = family.join(format!("trial-{trial}"));
            fs::create_dir_all(&output).map_err(io_error)?;
            context["active_trial"] = json!(trial);
            write(&family.join("manifest.json"), &context)?;
            let trial_started = std::time::Instant::now();
            let identity = super::journey_proof::execute_supplied_observed(
                opts,
                &paths.iso,
                built.iso_sha256.clone(),
                true,
                Some(&output),
            )?;
            let journey_ns = trial_started.elapsed().as_nanos();
            if git_head(&paths.root)? != source
                || tool_version("rustc", &["--version", "--verbose"])? != rustc
                || tool_version("qemu-system-x86_64", &["--version"])? != qemu
            {
                return Err(refusal(
                    "source/tool identities changed during native observation",
                ));
            }
            require_clean_source(&paths.root)?;
            let serial = fs::read_to_string(output.join("journey-serial.log")).map_err(io_error)?;
            let guest = super::native_timing::parse(&serial)?;
            let breakdown =
                breakdown::derive(&serial, &guest, &identity.host_id, &identity.boot_id)?;
            if trials
                .iter()
                .any(|prior: &serde_json::Value| prior["boot_id"] == identity.boot_id)
            {
                return Err(refusal("fresh process reused a prior Boot identity"));
            }
            if trials.iter().any(|prior| {
                prior["profile_id"] != identity.profile_id || prior["build_id"] != identity.build_id
            }) {
                return Err(refusal(
                    "fresh process changed its image profile or build identity",
                ));
            }
            let record = json!({"index":trial,"process":"fresh-qemu",
                "phase":if trial == 0 { "first-process" } else { "repeated-fresh-process" },
                "profile_id":identity.profile_id,"build_id":identity.build_id,
                "host_id":identity.host_id,"boot_id":identity.boot_id,"journey_ns":journey_ns,
                "guest":guest,"breakdown":breakdown,
                "frame_manifest":"journey-frames/manifest.json",
                "typing_observations":"journey-typing-timing.json","reading_observations":"journey-reading-timing.json"});
            write(&output.join("timing.json"), &record)?;
            trials.push(record);
            context["active_trial"] = serde_json::Value::Null;
            context["completed_trials"] = json!(trials);
            write(&family.join("manifest.json"), &context)?;
        }
        Ok(())
    })();
    context["total_ns"] = json!(started.elapsed().as_nanos());
    context["status"] = if result.is_ok() { "complete" } else { "failed" }.into();
    if result.is_err() {
        let excluded_start = context["active_trial"]
            .as_u64()
            .map_or(trials.len() as u64, |trial| trial + 1);
        context["excluded_trials"] = json!((excluded_start..u64::from(args.trials))
            .map(|trial| json!({"index":trial,"reason":"family-stopped-after-failure; no retry"}))
            .collect::<Vec<_>>());
    }
    if let Err(error) = &result {
        context["failure"] = json!({"reason":error.reason,"detail":error.detail});
    }
    write(&family.join("manifest.json"), &context)?;
    if !opts.quiet {
        println!("Native observation family: {}", family.display());
    }
    result
}

fn write(path: &Path, value: &serde_json::Value) -> Result<(), ConduitosError> {
    fs::write(
        path,
        serde_json::to_vec_pretty(value).map_err(|_| refusal("timing report encoding failed"))?,
    )
    .map_err(io_error)
}

fn refusal(detail: &str) -> ConduitosError {
    ConduitosError::refusal("native-latency-observation", detail)
}
fn io_error(error: std::io::Error) -> ConduitosError {
    ConduitosError::refusal("native-latency-observation-io", error.to_string())
}

fn require_clean_source(root: &Path) -> Result<(), ConduitosError> {
    let status = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=normal"])
        .current_dir(root)
        .output()
        .map_err(io_error)?;
    if !status.status.success() || !status.stdout.is_empty() {
        return Err(refusal(
            "source-bound observation requires a clean checkout",
        ));
    }
    Ok(())
}
fn tool_version(tool: &str, arguments: &[&str]) -> Result<String, ConduitosError> {
    let output = Command::new(tool)
        .args(arguments)
        .output()
        .map_err(io_error)?;
    if !output.status.success() || output.stdout.is_empty() || output.stdout.len() > 4096 {
        return Err(refusal("tool identity unavailable"));
    }
    String::from_utf8(output.stdout).map_err(|_| refusal("tool identity is not UTF-8"))
}

#[cfg(test)]
mod tests {
    use crate::cli::Cli;
    use clap::Parser;
    #[test]
    fn repetition_admission_refuses_zero_and_unbounded_families() {
        for count in ["1", "3", "8"] {
            assert!(Cli::try_parse_from([
                "xtask",
                "make",
                "conduitos",
                "native-latency-proof",
                "--trials",
                count
            ])
            .is_ok());
        }
        for count in ["0", "9", "256"] {
            assert!(Cli::try_parse_from([
                "xtask",
                "make",
                "conduitos",
                "native-latency-proof",
                "--trials",
                count
            ])
            .is_err());
        }
    }
}
