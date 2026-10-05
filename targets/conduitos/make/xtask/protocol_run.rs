//! Bounded emulator observation of an exact packaged protocol product image.
use super::{report::sha256_file, ConduitosError};
use crate::cli::GlobalOpts;
use clap::Args;
use serde_json::{json, Value};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
const MAXIMUM_TRANSCRIPT: u64 = 262144;

#[derive(Args, Debug)]
pub(super) struct RunArgs {
    /// Directory produced by protocol-image; the retained digest must match.
    #[arg(long)]
    media: PathBuf,
    /// New directory retaining serial, emulator diagnostics and observed outcome.
    #[arg(long)]
    output_dir: PathBuf,
    #[arg(long, default_value_t = 120, value_parser = clap::value_parser!(u64).range(1..=120))]
    seconds: u64,
}

pub(super) fn execute(args: RunArgs, opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        println!(
            "Observe exact protocol media {} for at most {} seconds",
            args.media.display(),
            args.seconds
        );
        return Ok(());
    }
    let receipt: Value =
        serde_json::from_slice(&read(&args.media.join("protocol-receipt.json"), 16384)?)
            .map_err(|error| refusal("protocol-run-receipt-refused", error))?;
    let image = args.media.join("conduitos.iso");
    let digest = sha256_file(&image)?;
    if receipt["schema"] != "conduit.conduitos/protocol-image-receipt@1"
        || receipt["image_sha256"] != digest
    {
        return Err(refusal(
            "protocol-run-image-refused",
            "exact packaged image digest required",
        ));
    }
    fs::create_dir(&args.output_dir)
        .map_err(|error| refusal("protocol-run-output-refused", error))?;
    let serial = args.output_dir.join("serial.log");
    let stderr = args.output_dir.join("qemu.log");
    let serial_target = format!("file:{}", serial.display());
    let mut child = Command::new("qemu-system-x86_64")
        .args([
            "-M",
            "q35",
            "-cpu",
            "max",
            "-m",
            "64M",
            "-smp",
            "1",
            "-display",
            "none",
            "-vga",
            "std",
            "-monitor",
            "none",
            "-serial",
            &serial_target,
            "-no-reboot",
            "-net",
            "none",
            "-device",
            "isa-debug-exit,iobase=0xf4,iosize=0x04",
            "-cdrom",
        ])
        .arg(&image)
        .args(["-boot", "d"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(
            fs::File::create(&stderr)
                .map_err(|error| refusal("protocol-run-output-refused", error))?,
        )
        .spawn()
        .map_err(|error| refusal("protocol-run-emulator-unavailable", error))?;
    let started = Instant::now();
    let observed = (|| loop {
        let bytes = if serial.exists() {
            read(&serial, MAXIMUM_TRANSCRIPT)?
        } else {
            vec![]
        };
        if fs::metadata(&stderr)
            .map_err(|error| refusal("protocol-run-diagnostics-refused", error))?
            .len()
            > MAXIMUM_TRANSCRIPT
        {
            return Err(refusal(
                "protocol-run-transcript-bound",
                "emulator diagnostics exceeded admission",
            ));
        }
        if let Some(outcome) = terminal(&bytes)? {
            return Ok(outcome);
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|error| refusal("protocol-run-observation-refused", error))?
        {
            return Err(refusal("protocol-run-ended-without-terminal", status));
        }
        if started.elapsed() >= Duration::from_secs(args.seconds) {
            return Err(refusal(
                "protocol-run-timeout",
                "no protocol terminal outcome observed",
            ));
        }
        thread::sleep(Duration::from_millis(50));
    })();
    // The observer always retires its isolated emulator, including observation
    // errors. A timeout is not evidence that native owners retired themselves.
    let _ = child.kill();
    let _ = child.wait();
    let outcome = observed?;
    let observed = json!({
        "schema": "conduit.conduitos/protocol-run-receipt@1",
        "proof_class": "freestanding-emulator",
        "image_sha256": digest,
        "outcome": outcome,
        "serial_sha256": sha256_file(&serial)?,
        "qemu_profile": "q35-single-cpu-64m-headless",
        "physical_compatibility": "unverified",
    });
    fs::write(
        args.output_dir.join("receipt.json"),
        serde_json::to_vec_pretty(&observed)
            .map_err(|error| refusal("protocol-run-receipt-refused", error))?,
    )
    .map_err(|error| refusal("protocol-run-output-refused", error))?;
    println!("Observed protocol outcome: {outcome}");
    Ok(())
}

fn terminal(bytes: &[u8]) -> Result<Option<String>, ConduitosError> {
    let text = String::from_utf8_lossy(bytes);
    for line in text
        .split_inclusive('\n')
        .filter(|line| line.ends_with('\n'))
    {
        let line = line.trim_end_matches(['\r', '\n']);
        if line.trim() == "CONDUIT_PROTOCOL_RETIRED complete" {
            if !text
                .lines()
                .any(|line| line.starts_with("CONDUIT_PROTOCOL_PLAN "))
                || !text
                    .lines()
                    .any(|line| line.starts_with("CONDUIT_PROTOCOL_PLAY "))
            {
                return Err(refusal(
                    "protocol-run-retirement-without-play",
                    "missing plan/play observations",
                ));
            }
            return Ok(Some("play-retired".into()));
        }
        if let Some(reason) = line.strip_prefix("CONDUIT_PROTOCOL_REFUSAL ") {
            return Ok(Some(format!("play-refused:{reason}")));
        }
        for (prefix, schema, stage) in [
            (
                "CONDUIT_BOOT_SIGN ",
                conduitos::sign_format::BOOT_SIGN_SCHEMA,
                "boot",
            ),
            (
                "CONDUIT_KERNEL_SIGN ",
                conduitos::sign_format::MACHINE_SIGN_SCHEMA,
                "root",
            ),
        ] {
            if let Some(json) = line.strip_prefix(prefix) {
                let sign: Value = serde_json::from_str(json)
                    .map_err(|error| refusal("protocol-run-sign-refused", error))?;
                if sign["schema"] == schema && sign["status"] == "refused" {
                    let reason = sign["reason"].as_str().ok_or_else(|| {
                        refusal("protocol-run-sign-refused", "missing refusal reason")
                    })?;
                    return Ok(Some(format!("{stage}-refused:{reason}")));
                }
            }
        }
    }
    Ok(None)
}
fn read(path: &Path, maximum: u64) -> Result<Vec<u8>, ConduitosError> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|error| refusal("protocol-run-read-refused", error))?
        .take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| refusal("protocol-run-read-refused", error))?;
    if bytes.len() as u64 > maximum {
        return Err(refusal("protocol-run-transcript-bound", path.display()));
    }
    Ok(bytes)
}
fn refusal(reason: &'static str, detail: impl std::fmt::Display) -> ConduitosError {
    ConduitosError::refusal(reason, detail.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refusal_and_retirement_are_distinct_and_retirement_requires_actual_play() {
        assert!(terminal(b"CONDUIT_PROTOCOL_RETIRED complete\n").is_err());
        assert_eq!(terminal(b"CONDUIT_PROTOCOL_PLAN source=s plan=p\nCONDUIT_PROTOCOL_PLAY play=x\nCONDUIT_PROTOCOL_RETIRED complete\n").unwrap().as_deref(), Some("play-retired"));
        assert_eq!(
            terminal(b"CONDUIT_PROTOCOL_REFUSAL protocol-retirement-refused\n")
                .unwrap()
                .as_deref(),
            Some("play-refused:protocol-retirement-refused")
        );
        assert_eq!(terminal(b"CONDUIT_KERNEL_SIGN {\"schema\":\"conduit.conduitos.kernel-sign/v2\",\"status\":\"refused\",\"reason\":\"protocol-controller-unavailable\"}\n").unwrap().as_deref(), Some("root-refused:protocol-controller-unavailable"));
        assert_eq!(terminal(b"CONDUIT_BOOT_SIGN {\"schema\":\"conduit.conduitos.boot-sign/v1\",\"status\":\"refused\",\"reason\":\"overlapping-artifacts\"}\n").unwrap().as_deref(), Some("boot-refused:overlapping-artifacts"));
        assert_eq!(terminal(b"CONDUIT_BOOT_STAGE ready\n").unwrap(), None);
        assert_eq!(terminal(b"CONDUIT_KERNEL_SIGN {\"schema\":").unwrap(), None);
    }
}
