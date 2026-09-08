use std::{
    fs,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use crate::cli::GlobalOpts;

use super::{build, image, profile::Paths, report::ArtifactRole, ConduitosArch, ConduitosError};

const PREFIX: &str = "CONDUIT_PROTECTION_SIGN ";

pub(super) fn execute(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    build::execute_isolation_proof(opts)?
        .artifact_role
        .require(ArtifactRole::ArchitectureProofAppliance)?;
    image::assemble_architecture_proof(ConduitosArch::X86_64, opts)?;
    if opts.dry_run {
        return Err(ConduitosError::refusal(
            "dry-run-has-no-protection-proof",
            "dry-run cannot manufacture processor-enforced evidence",
        ));
    }
    let paths = Paths::new(ConduitosArch::X86_64)?;
    let serial_path = paths.target.join("isolation-proof.log");
    let _ = fs::remove_file(&serial_path);
    let serial = format!("file:{}", serial_path.to_string_lossy());
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
            "-monitor",
            "none",
            "-serial",
            &serial,
            "-no-reboot",
            "-net",
            "none",
            "-device",
            "isa-debug-exit,iobase=0xf4,iosize=0x04",
            "-cdrom",
        ])
        .arg(&paths.iso)
        .args(["-boot", "d"])
        .current_dir(&paths.root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| ConduitosError::refusal("missing-qemu", error.to_string()))?;
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|error| {
            ConduitosError::refusal("isolation-proof-wait-failed", error.to_string())
        })? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(ConduitosError::refusal(
                "isolation-proof-timeout",
                fs::read_to_string(&serial_path).unwrap_or_default(),
            ));
        }
        thread::sleep(Duration::from_millis(10));
    };
    let transcript = fs::read_to_string(&serial_path).unwrap_or_default();
    if status.code() != Some(33) {
        return Err(ConduitosError::refusal(
            "isolation-proof-guest-failed",
            format!("{status}; {transcript}"),
        ));
    }
    let signs = transcript
        .lines()
        .filter_map(|line| line.strip_prefix(PREFIX))
        .collect::<Vec<_>>();
    if signs.len() != 1 {
        return Err(ConduitosError::refusal(
            "isolation-proof-sign-count",
            format!("expected one Sign, found {}; {transcript}", signs.len()),
        ));
    }
    let sign: serde_json::Value = serde_json::from_str(signs[0]).map_err(|error| {
        ConduitosError::refusal("isolation-proof-sign-invalid", error.to_string())
    })?;
    if sign["status"] != "completed"
        || sign["proof_class"] != "freestanding-emulator"
        || sign["architecture"] != "x86_64"
        || sign["privilege"] != "ring3"
        || sign["page_isolation"] != true
        || sign["io_bitmap_denied"] != true
        || sign["kernel_capability_table"] != true
        || sign["authorized_base_effect"] != true
        || sign["sibling_unchanged"] != true
        || sign["dma_isolation"] != false
        || sign["driver_isolation"] != false
        || sign["bounded"] != true
    {
        return Err(ConduitosError::refusal(
            "isolation-proof-sign-mismatch",
            sign.to_string(),
        ));
    }
    if !transcript.contains("CONDUIT_SERIAL_PRESENT protected-domain-authorized-effect") {
        return Err(ConduitosError::refusal(
            "isolation-proof-effect-absent",
            "authorized serial Base effect was not independently observed",
        ));
    }
    if !opts.quiet && !opts.json {
        println!("PROVED x86_64 ring-3 ConduitOS capability isolation");
    }
    Ok(())
}
