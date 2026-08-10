use std::{
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use crate::cli::GlobalOpts;

use super::{
    image,
    profile::{Paths, EXPECTED_QEMU_SUCCESS, LIMINE_VERSION, QEMU_PROFILE},
    report::{GuestBootSign, GuestKernelSign, GuestRun},
    ConduitosArch, ConduitosError,
};

pub fn execute(arch: ConduitosArch, opts: &GlobalOpts) -> Result<GuestRun, ConduitosError> {
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

pub(super) fn boot_once(paths: &Paths, opts: &GlobalOpts) -> Result<GuestRun, ConduitosError> {
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
    let kernel_signs: Vec<_> = serial
        .lines()
        .filter_map(|line| line.strip_prefix("CONDUIT_KERNEL_SIGN "))
        .collect();
    if kernel_signs.len() != 1 {
        return Err(ConduitosError::refusal(
            "malformed-kernel-sign",
            format!(
                "expected one structured kernel Sign, found {}",
                kernel_signs.len()
            ),
        ));
    }
    let boot: GuestBootSign = serde_json::from_str(signs[0])
        .map_err(|error| ConduitosError::refusal("malformed-boot-sign", error.to_string()))?;
    let kernel: GuestKernelSign = serde_json::from_str(kernel_signs[0])
        .map_err(|error| ConduitosError::refusal("malformed-kernel-sign", error.to_string()))?;
    validate_boot(&boot)?;
    validate_kernel(&boot, &kernel)?;
    if !opts.quiet && !opts.json {
        println!("{}", signs[0]);
        println!("{}", kernel_signs[0]);
    }
    Ok(GuestRun { boot, kernel })
}

fn validate_boot(sign: &GuestBootSign) -> Result<(), ConduitosError> {
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

fn validate_kernel(boot: &GuestBootSign, sign: &GuestKernelSign) -> Result<(), ConduitosError> {
    let valid_base_ids = sign.base_ids.len() == 7
        && sign.base_ids.iter().enumerate().all(|(index, id)| {
            id.len() == 64
                && id.bytes().all(|byte| byte.is_ascii_hexdigit())
                && !sign.base_ids[..index].contains(id)
        });
    let exact_id =
        |value: &str| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit());
    if sign.schema != "conduit.conduitos.kernel-sign/v2"
        || sign.status != "accepted"
        || sign.arch != "x86_64"
        || sign.build_id != boot.build_id
        || sign.kernel != "conduit-kernel"
        || sign.scheduler_profile != "conduitos/single-lane-cooperative@1"
        || sign.host_id != boot.host_id
        || sign.boot_id != boot.boot_id
        || sign.pipeline != "check-plan-lower-kernel"
        || !exact_id(&sign.source_document_id)
        || !exact_id(&sign.checked_form_id)
        || !exact_id(&sign.expanded_form_id)
        || !exact_id(&sign.plan_id)
        || !exact_id(&sign.fragment_id)
        || !exact_id(&sign.active_play_id)
        || sign.planned_sign_items == 0
        || sign.planned_sign_bytes == 0
        || sign.cord_item_capacity != 1
        || sign.cord_byte_capacity != 8
        || sign.semantic_result != "tick-sequence-0"
        || sign.allocation_before_play == 0
        || sign.allocation_before_play != sign.allocation_after_play
        || sign.allocation_capacity != boot.runtime_arena_bytes as usize
        || !sign.allocation_stable_during_play
        || sign.base_count != 7
        || !valid_base_ids
        || sign.memory_arena_bytes != boot.runtime_arena_bytes
        || sign.execution_lanes != 1
        || sign.timer_slots != 1
        || sign.serial_slots != 1
        || sign.serial_maximum_bytes != 16
        || sign.interrupt_fact_slots != 4
        || sign.sign_item_slots != 64
        || sign.logical_operations != 2
        || sign.kernel_decisions == 0
        || sign.kernel_signs == 0
        || sign.timer_irq_wakes != 1
        || sign.idle_entries == 0
        || sign.serial_presentations != 1
        || !sign.clock_monotonic
        || sign.pending_host_operations != 0
        || !sign.sse2
    {
        return Err(ConduitosError::refusal(
            "invalid-kernel-sign",
            format!("kernel Sign failed exact validation: {sign:?}"),
        ));
    }
    Ok(())
}
