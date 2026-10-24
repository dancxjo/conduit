//! Pinned emulator acceptance for the IA-32 product Host artifact.

use std::{
    fs,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use crate::cli::GlobalOpts;

use super::{
    ConduitosArch, ConduitosError, ia32_a1,
    profile::Paths,
    report::{git_head, sha256_file},
};

const PREFIX: &str = "CONDUIT_IA32_PRODUCT ";
const OBSERVATORY_PREFIX: &str = "CONDUIT_OBSERVATORY_SNAPSHOT ";

mod clock;
mod framebuffer;
mod graphical;
pub(super) use clock::GuestClock;

#[derive(Clone, Copy)]
enum FirmwareMode {
    Uefi32,
    LegacyBios,
}

impl FirmwareMode {
    const fn machine(self) -> &'static str {
        match self {
            Self::Uefi32 => "q35",
            Self::LegacyBios => "pc",
        }
    }

    const fn expected_firmware(self) -> &'static str {
        match self {
            Self::Uefi32 => "uefi32",
            Self::LegacyBios => "x86-bios",
        }
    }
}

pub(super) fn boot_legacy_bios(
    image: &std::path::Path,
    expected_profile_id: &str,
    expected_build_id: &str,
    expected_image_binding: &str,
    clock: GuestClock,
    opts: &GlobalOpts,
) -> Result<(), ConduitosError> {
    let (product, observatory) = boot_once(
        image,
        expected_profile_id,
        expected_build_id,
        expected_image_binding,
        opts,
        "legacy-bios-only",
        (FirmwareMode::LegacyBios, clock),
    )?;
    let paths = Paths::new(ConduitosArch::Ia32)?;
    let proof = serde_json::json!({
        "schema": "conduit.conduitos/ia32-legacy-bios-product-proof@1",
        "proof_class": "freestanding-ia32-legacy-bios-emulator",
        "base_commit": git_head(&paths.root)?,
        "image_sha256": sha256_file(image)?,
        "firmware_environment": "x86-bios",
        "emulator_cpu": "max",
        "carrier": "limine-hybrid-iso-legacy-bios-entry",
        "product": product,
        "observatory": observatory,
        "physical_machine_booted": false,
        "legacy_bios_vga_text_receipt": true,
        "guest_clock": clock.label(),
        "stopped_by_harness": true
    });
    fs::write(
        paths.target.join("ia32-legacy-bios-product-proof.json"),
        serde_json::to_vec_pretty(&proof)
            .map_err(|error| refusal("ia32-product-proof-invalid", error.to_string()))?,
    )
    .map_err(|error| refusal("ia32-product-proof-unavailable", error.to_string()))?;
    Ok(())
}

pub(super) fn boot_twice(
    image: &std::path::Path,
    expected_profile_id: &str,
    expected_build_id: &str,
    expected_image_binding: &str,
    opts: &GlobalOpts,
) -> Result<(), ConduitosError> {
    let image_sha256 = sha256_file(image)?;
    let (first, first_observatory) = boot_once(
        image,
        expected_profile_id,
        expected_build_id,
        expected_image_binding,
        opts,
        "first",
        (FirmwareMode::Uefi32, GuestClock::Host),
    )?;
    let (second, second_observatory) = boot_once(
        image,
        expected_profile_id,
        expected_build_id,
        expected_image_binding,
        opts,
        "second",
        (FirmwareMode::Uefi32, GuestClock::Host),
    )?;
    if first.get("graphical").is_some() {
        return graphical::finish_twice(
            image,
            &image_sha256,
            [(first, first_observatory), (second, second_observatory)],
        );
    }
    let (legacy_bios, legacy_bios_observatory) = boot_once(
        image,
        expected_profile_id,
        expected_build_id,
        expected_image_binding,
        opts,
        "legacy-bios",
        (FirmwareMode::LegacyBios, GuestClock::Host),
    )?;
    if first["host_id"] == second["host_id"]
        || first["boot_id"] == second["boot_id"]
        || legacy_bios["host_id"] == first["host_id"]
        || legacy_bios["host_id"] == second["host_id"]
        || legacy_bios["boot_id"] == first["boot_id"]
        || legacy_bios["boot_id"] == second["boot_id"]
    {
        return Err(refusal(
            "stale-ia32-product-identity",
            "independent product boots reused HostId or BootId",
        ));
    }
    if sha256_file(image)? != image_sha256 {
        return Err(refusal(
            "ia32-product-image-changed",
            "the product image changed between firmware-carrier boots",
        ));
    }
    let paths = Paths::new(ConduitosArch::Ia32)?;
    let snapshot_path = paths.target.join("ia32-product-observatory.json");
    fs::write(
        &snapshot_path,
        serde_json::to_vec_pretty(&first_observatory)
            .map_err(|error| refusal("ia32-product-proof-invalid", error.to_string()))?,
    )
    .map_err(|error| refusal("ia32-product-proof-unavailable", error.to_string()))?;
    prove_patchbay(&snapshot_path, &first)?;
    let proof = serde_json::json!({
        "schema": "conduit.conduitos/ia32-product-proof@2",
        "proof_class": "freestanding-ia32-emulator-dual-firmware-carrier",
        "emulator_cpu": "max",
        "base_commit": git_head(&paths.root)?,
        "image_sha256": image_sha256,
        "first": first,
        "second": second,
        "legacy_bios": legacy_bios,
        "first_observatory": first_observatory,
        "second_observatory": second_observatory,
        "legacy_bios_observatory": legacy_bios_observatory,
        "fresh_host_id": true,
        "fresh_boot_id": true,
        "uefi32_booted": true,
        "legacy_bios_booted": true,
        "legacy_bios_vga_text_receipt": true,
        "same_immutable_image": true,
        "native_patchbay_consumed": true,
        "patchbay_projection": "shared-workbench-library",
        "stopped_by_harness": true
    });
    fs::write(
        paths.target.join("ia32-product-proof.json"),
        serde_json::to_vec_pretty(&proof)
            .map_err(|error| refusal("ia32-product-proof-invalid", error.to_string()))?,
    )
    .map_err(|error| refusal("ia32-product-proof-unavailable", error.to_string()))?;
    Ok(())
}

fn prove_patchbay(
    snapshot: &std::path::Path,
    product: &serde_json::Value,
) -> Result<(), ConduitosError> {
    let linear = super::product_patchbay::render(snapshot, "patchbay-rejected-ia32-product")?;
    let base_count = 7 + usize::from(product["framebuffer_observation"]["framebuffer"].is_object());
    let expected_bases = format!("BASES {base_count}");
    for required in [
        product["host_id"].as_str().unwrap_or_default(),
        product["boot_id"].as_str().unwrap_or_default(),
        product["ordinary_plan_id"].as_str().unwrap_or_default(),
        product["ordinary_play_id"].as_str().unwrap_or_default(),
        expected_bases.as_str(),
        "lifecycle=Completed",
        "proof=FreestandingEmulator",
    ] {
        if required.is_empty() || !linear.contains(required) {
            return Err(refusal(
                "patchbay-ia32-product-projection-incomplete",
                format!("native Patchbay omitted {required}"),
            ));
        }
    }
    Ok(())
}

fn boot_once(
    image: &std::path::Path,
    expected_profile_id: &str,
    expected_build_id: &str,
    expected_image_binding: &str,
    opts: &GlobalOpts,
    run: &str,
    (firmware_mode, clock): (FirmwareMode, GuestClock),
) -> Result<(serde_json::Value, serde_json::Value), ConduitosError> {
    let paths = Paths::new(ConduitosArch::Ia32)?;
    let graphical =
        graphical::Selected::prepare(image, &paths, matches!(firmware_mode, FirmwareMode::Uefi32))?;
    let transcript_path = graphical.as_ref().map_or_else(
        || paths.target.join(format!("ia32-product-{run}.log")),
        |g| g.capture.serial_path(),
    );
    // Unix socket paths have a small fixed limit for both QEMU and the client.
    // Keep this ephemeral endpoint independent of the checkout's path length.
    let monitor_path =
        std::env::temp_dir().join(format!("conduit-ia32-{}.sock", std::process::id()));
    let vga_path = paths.target.join(format!("ia32-product-{run}-vga.bin"));
    fs::write(&transcript_path, [])
        .map_err(|error| refusal("ia32-product-boot-failed", error.to_string()))?;
    for stale in [&monitor_path, &vga_path] {
        if stale.exists() {
            fs::remove_file(stale)
                .map_err(|error| refusal("ia32-product-boot-failed", error.to_string()))?;
        }
    }
    let mut child = Command::new("qemu-system-i386");
    child
        .args([
            "-machine",
            firmware_mode.machine(),
            "-cpu",
            "max",
            "-m",
            "512M",
            "-smp",
            "1",
            "-display",
            "none",
            "-serial",
            "none",
            "-net",
            "none",
            "-no-reboot",
            "-debugcon",
        ])
        .arg(format!("file:{}", transcript_path.display()))
        .args(["-global", "isa-debugcon.iobase=0xe9"]);
    clock.configure(&mut child);
    if matches!(firmware_mode, FirmwareMode::LegacyBios) {
        child
            .arg("-monitor")
            .arg(format!("unix:{},server,nowait", monitor_path.display()));
    } else {
        child.args(["-monitor", "none"]);
    }
    if matches!(firmware_mode, FirmwareMode::Uefi32) {
        let (firmware, vars_template) = ia32_a1::firmware_paths(&paths)?;
        let vars = paths.target.join(format!("ia32-product-{run}-vars.fd"));
        super::ia32_firmware::copy_vars(&vars_template, &vars)
            .map_err(|error| refusal("unavailable-ia32-firmware", error.to_string()))?;
        child
            .arg("-drive")
            .arg(format!(
                "if=pflash,format=raw,readonly=on,file={}",
                firmware.display()
            ))
            .arg("-drive")
            .arg(format!("if=pflash,format=raw,file={}", vars.display()));
    }
    child
        .arg("-cdrom")
        .arg(image)
        .args(["-boot", "d"])
        .current_dir(&paths.root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    if let Some(graphical) = &graphical {
        graphical.record(&mut child, &paths, image, run)?;
    }
    let mut child = child
        .spawn()
        .map_err(|error| refusal("unavailable-ia32-emulator", error.to_string()))?;
    let started = Instant::now();
    let deadline = started + Duration::from_secs(60);
    loop {
        if fs::metadata(&transcript_path).is_ok_and(|m| m.len() > 1024 * 1024) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(refusal("ia32-product-serial-bound", "serial exceeds 1 MiB"));
        }
        let transcript = fs::read_to_string(&transcript_path).unwrap_or_default();
        if let (Some(json), Some(observatory_json), true) = (
            complete_line(&transcript, PREFIX),
            complete_line(&transcript, OBSERVATORY_PREFIX),
            matches!(firmware_mode, FirmwareMode::Uefi32)
                || super::ia32_vga_receipt::completed_boot(&transcript).is_some(),
        ) {
            let captured = (|| {
                let mut value: serde_json::Value = serde_json::from_str(json)
                    .map_err(|error| refusal("malformed-ia32-product-sign", error.to_string()))?;
                validate_sign(
                    &value,
                    expected_profile_id,
                    expected_build_id,
                    expected_image_binding,
                )?;
                let observatory: serde_json::Value = serde_json::from_str(observatory_json)
                    .map_err(|error| refusal("malformed-ia32-observatory", error.to_string()))?;
                validate_observatory(&observatory, &value, firmware_mode.expected_firmware())?;
                value["framebuffer_observation"] =
                    framebuffer::capture(&transcript, &value, &observatory, graphical.is_some())?;
                if let Some(graphical) = &graphical {
                    value["graphical"] =
                        graphical.finish(&mut child, &transcript, &value, started)?;
                }
                let ordinary_plan: conduit_core::Plan =
                    serde_json::from_value(observatory["plans"][0].clone()).map_err(|error| {
                        refusal("ordinary-product-plan-invalid", error.to_string())
                    })?;
                value["ordinary_source_conformance"] =
                    super::ordinary_source_conformance::capture(&ordinary_plan)?;
                value["ordinary_domain_cost"] =
                    super::protected_product_receipt::capture(&transcript, &value, "ia32")?;
                if matches!(firmware_mode, FirmwareMode::LegacyBios) {
                    super::ia32_vga_receipt::validate_completion(&transcript, &value).and_then(
                        |()| {
                            super::ia32_vga_receipt::capture_and_validate(
                                &monitor_path,
                                &vga_path,
                                &value,
                            )
                        },
                    )?;
                }
                Ok::<_, ConduitosError>((value, observatory))
            })();
            let (value, observatory) = match captured {
                Ok(captured) => captured,
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(error);
                }
            };
            thread::sleep(Duration::from_millis(250));
            if child
                .try_wait()
                .map_err(|error| refusal("ia32-product-wait-failed", error.to_string()))?
                .is_some()
            {
                return Err(refusal(
                    "ia32-product-not-long-lived",
                    "product Host exited after ready Sign",
                ));
            }
            child
                .kill()
                .and_then(|_| child.wait().map(|_| ()))
                .map_err(|error| refusal("ia32-product-stop-failed", error.to_string()))?;
            if !opts.quiet && !opts.json {
                println!(
                    "BOOTED {} to IA-32 linear product front door",
                    image.display()
                );
            }
            return Ok((value, observatory));
        }
        if child
            .try_wait()
            .map_err(|error| refusal("ia32-product-wait-failed", error.to_string()))?
            .is_some()
        {
            return Err(refusal("ia32-product-exited-early", transcript));
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(refusal("ia32-product-timeout", transcript));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn complete_line<'a>(transcript: &'a str, prefix: &str) -> Option<&'a str> {
    super::emitted_line::complete_json_line(transcript, prefix)
}

fn validate_observatory(
    value: &serde_json::Value,
    product: &serde_json::Value,
    expected_firmware: &str,
) -> Result<(), ConduitosError> {
    let hosts = value["hosts"]
        .as_array()
        .ok_or_else(|| refusal("invalid-ia32-observatory", "hosts absent"))?;
    let plans = value["plans"]
        .as_array()
        .ok_or_else(|| refusal("invalid-ia32-observatory", "plans absent"))?;
    let plays = value["plays"]
        .as_array()
        .ok_or_else(|| refusal("invalid-ia32-observatory", "plays absent"))?;
    let sealed = value["sealed_boot_provenance"]
        .as_array()
        .ok_or_else(|| refusal("invalid-ia32-observatory", "boot provenance absent"))?;
    if value["schema"] != "conduit.observatory.snapshot/v2"
        || hosts.len() != 1
        || plans.len() != 1
        || plays.len() != 1
        || sealed.len() != 1
        || hosts[0]["advertisement"]["host_id"] != product["host_id"]
        || hosts[0]["advertisement"]["boot_id"] != product["boot_id"]
        || plans[0]["plan_id"] != product["ordinary_plan_id"]
        || plays[0]["active_play_id"] != product["ordinary_play_id"]
        || plays[0]["boot_id"] != product["boot_id"]
        || sealed[0]["host_id"] != product["host_id"]
        || sealed[0]["boot_id"] != product["boot_id"]
        || sealed[0]["firmware_environment"] != expected_firmware
        || sealed[0]["build_id"] != product["build_id"]
        || sealed[0]["image_id"] != product["image_id"]
        || sealed[0]["proof_class"] != "FreestandingEmulator"
    {
        return Err(refusal("invalid-ia32-observatory", value.to_string()));
    }
    Ok(())
}

fn validate_sign(
    value: &serde_json::Value,
    expected_profile_id: &str,
    expected_build_id: &str,
    expected_image_binding: &str,
) -> Result<(), ConduitosError> {
    if value["schema"] != "conduit.conduitos/ia32-product@1"
        || value["status"] != "ready"
        || value["profile_id"] != expected_profile_id
        || value["build_id"] != expected_build_id
        || value["image_id"] != expected_image_binding
        || value["host_id"].as_str().is_none_or(str::is_empty)
        || value["boot_id"].as_str().is_none_or(str::is_empty)
        || value["body_id"] != serde_json::Value::Null
        || value["interactive_local_control"] != false
        || value["long_lived"] != true
        || value["semantic_result"] != "HELLO, CONDUITOS"
        || value["presenter_implementation_id"] != "presenter/ia32-linear-debugcon@1"
        || value["ordinary_plan_id"].as_str().is_none_or(str::is_empty)
        || value["ordinary_play_id"].as_str().is_none_or(str::is_empty)
    {
        return Err(refusal("profile-built-make-mismatch", value.to_string()));
    }
    Ok(())
}

fn refusal(reason: &'static str, detail: impl Into<String>) -> ConduitosError {
    ConduitosError::refusal(reason, detail)
}

#[cfg(test)]
mod tests;
