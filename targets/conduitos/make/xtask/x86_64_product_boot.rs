//! Exact x86_64 product-image boot and reboot into the zero-Body Crèche.

use super::{
    hid_qmp,
    profile::Paths,
    report::{git_head, sha256_file, GuestBootSign},
    ConduitosArch, ConduitosError,
};
use crate::cli::GlobalOpts;
use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

pub(super) fn boot_twice(
    image: &Path,
    profile: &str,
    build: &str,
    binding: &str,
    opts: &GlobalOpts,
) -> Result<(), ConduitosError> {
    let first = boot_once(image, profile, build, binding, opts)?;
    let second = boot_once(image, profile, build, binding, opts)?;
    if first["boot_id"] == second["boot_id"] || first["host_id"] == second["host_id"] {
        return Err(ConduitosError::refusal(
            "stale-x86_64-product-identity",
            "independent product boots reused HostId or BootId",
        ));
    }
    let paths = Paths::new(ConduitosArch::X86_64)?;
    let proof = serde_json::json!({
        "schema":"conduit.conduitos/x86_64-product-proof@1", "base_commit":git_head(&paths.root)?,
        "image_sha256":sha256_file(image)?, "first":first, "second":second,
        "fresh_host_id":true, "fresh_boot_id":true, "stopped_by_harness":true
    });
    fs::write(
        paths.target.join("x86_64-product-proof.json"),
        serde_json::to_vec_pretty(&proof).map_err(|error| {
            ConduitosError::refusal("x86_64-product-proof-invalid", error.to_string())
        })?,
    )
    .map_err(|error| ConduitosError::refusal("x86_64-product-proof-unavailable", error.to_string()))
}

fn boot_once(
    image: &Path,
    profile: &str,
    build: &str,
    binding: &str,
    opts: &GlobalOpts,
) -> Result<serde_json::Value, ConduitosError> {
    let paths = Paths::new(ConduitosArch::X86_64)?;
    let serial_path = paths.target.join("profile-built-boot.log");
    let socket = paths.target.join("profile-built-boot.qmp");
    let _ = fs::remove_file(&serial_path);
    let _ = fs::remove_file(&socket);
    let serial = format!("file:{}", serial_path.to_string_lossy());
    let qmp_arg = format!("unix:{},server=on,wait=off", socket.to_string_lossy());
    let image = image.to_str().ok_or_else(|| {
        ConduitosError::refusal("profile-built-image-path-invalid", "non-UTF-8 path")
    })?;
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
            "-qmp",
            &qmp_arg,
            "-serial",
            &serial,
            "-no-reboot",
            "-net",
            "none",
            "-device",
            "qemu-xhci,id=conduitos-xhci,p2=1,p3=0",
            "-device",
            "usb-kbd,bus=conduitos-xhci.0,port=1",
            "-cdrom",
            image,
            "-boot",
            "d",
        ])
        .current_dir(&paths.root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| ConduitosError::refusal("missing-qemu", error.to_string()))?;
    hid_qmp::wait_for_stage(
        &serial_path,
        &mut child,
        "CONDUIT_CRECHE_CHECKPOINT ready",
        "profile-built-creche-ready-timeout",
    )?;
    let deadline = Instant::now() + Duration::from_secs(20);
    let (journey_json, boot_json) = loop {
        if let Ok(serial) = fs::read_to_string(&serial_path) {
            let ready = serial
                .lines()
                .filter(|line| *line == "CONDUIT_BOOT_STAGE front-door-ready")
                .count();
            let journeys = serial
                .lines()
                .filter_map(|line| line.strip_prefix("CONDUIT_PRODUCT_JOURNEY "))
                .collect::<Vec<_>>();
            let boots = serial
                .lines()
                .filter_map(|line| line.strip_prefix("CONDUIT_BOOT_SIGN "))
                .collect::<Vec<_>>();
            if ready == 1 && journeys.len() == 1 && boots.len() == 1 {
                break (journeys[0].to_owned(), boots[0].to_owned());
            }
            if ready > 1 || journeys.len() > 1 || boots.len() > 1 {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ConduitosError::refusal("malformed-profile-built-boot-stage", format!("expected one front-door stage, product journey, and Boot Sign; found {ready}, {}, and {}", journeys.len(), boots.len())));
            }
        }
        if child
            .try_wait()
            .map_err(|error| {
                ConduitosError::refusal("profile-built-boot-failed", error.to_string())
            })?
            .is_some()
        {
            return Err(ConduitosError::refusal(
                "profile-built-boot-failed",
                "QEMU exited before reaching its presented front door",
            ));
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(ConduitosError::refusal(
                "profile-built-boot-timeout",
                "QEMU did not reach its presented front door within 20 seconds",
            ));
        }
        thread::sleep(Duration::from_millis(10));
    };
    child
        .kill()
        .and_then(|_| child.wait().map(|_| ()))
        .map_err(|error| {
            ConduitosError::refusal("profile-built-boot-stop-failed", error.to_string())
        })?;
    let journey: serde_json::Value = serde_json::from_str(&journey_json).map_err(|error| {
        ConduitosError::refusal("malformed-profile-built-product-journey", error.to_string())
    })?;
    let boot: GuestBootSign = serde_json::from_str(&boot_json).map_err(|error| {
        ConduitosError::refusal("malformed-profile-built-boot-sign", error.to_string())
    })?;
    validate(&journey, &boot, profile, build, binding)?;
    if !opts.quiet && !opts.json {
        println!(
            "BOOTED {image} to presented front door on Boot {}",
            boot.boot_id
        );
    }
    Ok(
        serde_json::json!({"host_id":boot.host_id,"boot_id":boot.boot_id,"journey_status":journey["status"],"presenter_implementation_id":journey["presenter_implementation_id"],"profile_id":boot.profile_id,"build_id":boot.build_id,"image_binding":boot.image_binding}),
    )
}

fn validate(
    journey: &serde_json::Value,
    boot: &GuestBootSign,
    profile: &str,
    build: &str,
    image: &str,
) -> Result<(), ConduitosError> {
    let boot_value = serde_json::to_value(boot).map_err(|error| {
        ConduitosError::refusal("malformed-profile-built-boot-sign", error.to_string())
    })?;
    for (record, value, field, expected) in [
        (
            "boot",
            &boot_value,
            "schema",
            serde_json::json!("conduit.conduitos.boot-sign/v1"),
        ),
        ("boot", &boot_value, "status", serde_json::json!("accepted")),
        ("boot", &boot_value, "arch", serde_json::json!("x86_64")),
        (
            "boot",
            &boot_value,
            "profile_id",
            serde_json::json!(profile),
        ),
        ("boot", &boot_value, "build_id", serde_json::json!(build)),
        (
            "boot",
            &boot_value,
            "image_binding",
            serde_json::json!(image),
        ),
        (
            "boot",
            &boot_value,
            "offer_generation",
            serde_json::json!(1),
        ),
        ("journey", journey, "status", serde_json::json!("world")),
        ("journey", journey, "profile_id", serde_json::json!(profile)),
        ("journey", journey, "build_id", serde_json::json!(build)),
        ("journey", journey, "image_id", serde_json::json!(image)),
        (
            "journey",
            journey,
            "host_id",
            serde_json::json!(boot.host_id),
        ),
        (
            "journey",
            journey,
            "boot_id",
            serde_json::json!(boot.boot_id),
        ),
        (
            "journey",
            journey,
            "presenter_implementation_id",
            serde_json::json!("presentation/renderer-conduitos-native@1"),
        ),
        ("journey", journey, "body_id", serde_json::Value::Null),
        ("journey", journey, "plan_id", serde_json::Value::Null),
        (
            "journey",
            journey,
            "active_play_id",
            serde_json::Value::Null,
        ),
    ] {
        if value.get(field) != Some(&expected) {
            return Err(ConduitosError::refusal(
                "profile-built-make-mismatch",
                format!(
                    "{record}.{field}: expected {expected}, observed {}",
                    value
                        .get(field)
                        .map_or_else(|| "<missing>".into(), ToString::to_string)
                ),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "x86_64_product_boot_tests.rs"]
mod tests;
