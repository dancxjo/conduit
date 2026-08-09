use std::{
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use crate::cli::GlobalOpts;

use super::{
    image,
    profile::{Paths, EXPECTED_QEMU_SUCCESS, LIMINE_VERSION, QEMU_PROFILE},
    report::GuestBootSign,
    ConduitosArch, ConduitosError,
};

pub fn execute(arch: ConduitosArch, opts: &GlobalOpts) -> Result<GuestBootSign, ConduitosError> {
    let paths = Paths::new(arch)?;
    let _image = image::execute(arch, opts)?;
    if opts.dry_run {
        println!("qemu-system-x86_64 {QEMU_PROFILE}");
        return Err(ConduitosError::refusal(
            "dry-run-has-no-boot-sign",
            "run/prove dry-run cannot manufacture execution evidence",
        ));
    }
    boot_once(&paths, opts)
}

pub(super) fn boot_once(paths: &Paths, opts: &GlobalOpts) -> Result<GuestBootSign, ConduitosError> {
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
            "none",
            "-monitor",
            "none",
            "-serial",
            "stdio",
            "-no-reboot",
            "-no-shutdown",
            "-net",
            "none",
            "-rtc",
            "base=2026-08-09T00:00:00,clock=vm",
            "-device",
            "isa-debug-exit,iobase=0xf4,iosize=0x04",
            "-cdrom",
            paths.iso.to_str().unwrap(),
            "-boot",
            "d",
        ])
        .current_dir(&paths.root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            ConduitosError::refusal(
                "missing-qemu",
                format!("cannot launch qemu-system-x86_64: {error}"),
            )
        })?;
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        match child.try_wait().map_err(|error| {
            ConduitosError::refusal("qemu-boot-failed", format!("cannot wait for QEMU: {error}"))
        })? {
            Some(status) => break status,
            None if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            None => {
                child.kill().map_err(|error| {
                    ConduitosError::refusal(
                        "qemu-timeout",
                        format!("cannot stop timed-out QEMU: {error}"),
                    )
                })?;
                let _ = child.wait();
                return Err(ConduitosError::refusal(
                    "qemu-timeout",
                    "QEMU did not emit a terminal debug-exit within 20 seconds",
                ));
            }
        }
    };
    let output = child.wait_with_output().map_err(|error| {
        ConduitosError::refusal(
            "qemu-boot-failed",
            format!("cannot collect QEMU output: {error}"),
        )
    })?;
    if status.code() != Some(EXPECTED_QEMU_SUCCESS) {
        return Err(ConduitosError::refusal(
            "qemu-boot-failed",
            format!(
                "expected isa-debug-exit status {EXPECTED_QEMU_SUCCESS}, got {}; serial: {}",
                status,
                String::from_utf8_lossy(&output.stdout)
            ),
        ));
    }
    let serial = String::from_utf8(output.stdout).map_err(|error| {
        ConduitosError::refusal("malformed-boot-sign", format!("non-UTF-8 serial: {error}"))
    })?;
    let signs: Vec<_> = serial
        .lines()
        .filter_map(|line| line.strip_prefix("CONDUIT_BOOT_SIGN "))
        .collect();
    if signs.len() != 1 {
        return Err(ConduitosError::refusal(
            "malformed-boot-sign",
            format!("expected one structured boot Sign, found {}", signs.len()),
        ));
    }
    let sign: GuestBootSign = serde_json::from_str(signs[0])
        .map_err(|error| ConduitosError::refusal("malformed-boot-sign", error.to_string()))?;
    validate(&sign)?;
    if !opts.quiet && !opts.json {
        println!("{}", signs[0]);
    }
    Ok(sign)
}

fn validate(sign: &GuestBootSign) -> Result<(), ConduitosError> {
    if sign.schema != "conduit.conduitos.boot-sign/v1"
        || sign.status != "accepted"
        || sign.arch != "x86_64"
        || sign.limine != LIMINE_VERSION
        || sign.qemu_profile != QEMU_PROFILE
        || sign.host_id.len() != 64
        || sign.boot_id.len() != 64
        || sign.memory_regions == 0
        || sign.runtime_arena_bytes != 262_144
    {
        return Err(ConduitosError::refusal(
            "invalid-boot-sign",
            format!("boot Sign failed exact validation: {sign:?}"),
        ));
    }
    Ok(())
}
