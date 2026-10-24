//! Held normal PROFILE graphical collector; not yet wired or product-tested.
use super::super::{
    emitted_line, loongarch64_a1, loongarch64_emulator, loongarch64_entropy_input, report,
    target_build::graphical::Capture, ConduitosArch, ConduitosError,
};
use super::{refusal, Paths, OBSERVATORY_PREFIX, PRODUCT_PREFIX};
use serde_json::Value;
use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub(super) fn boot_if_selected(
    paths: &Paths,
    image: &Path,
) -> Result<Option<(String, Value)>, ConduitosError> {
    let manifest_path = image
        .parent()
        .unwrap_or(Path::new("."))
        .join("build-manifest.json");
    if !manifest_path.exists() {
        return Ok(None);
    }
    if fs::metadata(&manifest_path).map_err(io)?.len() > 1024 * 1024 {
        return Err(io("manifest exceeds 1 MiB"));
    }
    let manifest: Value =
        serde_json::from_slice(&fs::read(manifest_path).map_err(io)?).map_err(io)?;
    if !manifest["resolved_build"]["presenters"]
        .as_array()
        .is_some_and(|p| p.iter().any(|p| p == "presenter/native-graphical@1"))
    {
        return Ok(None);
    }
    if manifest["artifact_role"] != "product-host"
        || manifest["target"] != "conduitos/loongarch64/virt"
        || manifest["image"]["sha256"] != report::sha256_file(image)?
    {
        return Err(io("wrong graphical target, role or image bytes"));
    }
    let source = manifest["source_identity"]
        .as_str()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| io("source identity missing"))?;
    let capture = Capture::prepare(&paths.target, ConduitosArch::Loongarch64)?;
    let qemu = loongarch64_emulator::selected_graphical(paths)?;
    // Make this existing pinned firmware helper visible during gated wiring;
    // the held file is deliberately not compiled before that admission.
    let firmware = loongarch64_a1::prepare_firmware(paths)?;
    let firmware_hash = report::sha256_file(&firmware)?;
    let entropy = loongarch64_entropy_input::Input::acquire(&paths.target)?;
    let mut command = Command::new(&qemu);
    command
        .args([
            "-machine",
            "virt",
            "-cpu",
            "la464",
            "-m",
            "2G",
            "-smp",
            "1",
            "-accel",
            "tcg,thread=single",
            "-bios",
        ])
        .arg(&firmware)
        .args(["-fw_cfg", &entropy.argument()?, "-drive"])
        .arg(format!(
            "file={},format=raw,if=none,id=boot,readonly=on",
            image.display()
        ))
        .args([
            "-device",
            "virtio-blk-pci,drive=boot,bootindex=0",
            "-device",
            "ramfb",
            "-display",
            "none",
            "-monitor",
            "none",
            "-serial",
        ])
        .arg(format!("file:{}", capture.serial_path().display()))
        .args(["-net", "none", "-qmp"])
        .arg(capture.endpoint())
        .arg("-no-reboot")
        .env("QEMU_AUDIO_DRV", "none")
        .env_remove("QEMU_MODULE_DIR");
    capture.record_launch(&command, &paths.root, &[&firmware], image, source)?;
    let started = Instant::now();
    let mut child = command
        .current_dir(&paths.root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(fs::File::create(capture.stderr_path()).map_err(io)?)
        .spawn()
        .map_err(io)?;
    let result = (|| loop {
        if fs::metadata(capture.serial_path()).is_ok_and(|m| m.len() > 1024 * 1024) {
            return Err(io("serial exceeds 1 MiB"));
        }
        let bytes = fs::read(capture.serial_path()).unwrap_or_default();
        let transcript = String::from_utf8_lossy(&bytes);
        if let Some(reason) = transcript
            .split("CONDUIT_RISCV64_PRODUCT_REFUSAL ")
            .nth(1)
            .and_then(|s| s.lines().next())
        {
            return Err(refusal("loongarch64-product-refused", reason));
        }
        if let (Some(product), Some(_)) = (
            emitted_line::complete_json_line(&transcript, PRODUCT_PREFIX),
            emitted_line::complete_json_line(&transcript, OBSERVATORY_PREFIX),
        ) {
            let product: Value = serde_json::from_str(product).map_err(io)?;
            let proof = capture.finish_static(&mut child, &transcript, &product, started)?;
            validate_mapping(&proof)?;
            return Ok(Some((transcript.into_owned(), proof)));
        }
        if started.elapsed() >= Duration::from_secs(45) {
            return Err(io("graphical product boot deadline exceeded"));
        }
        if let Some(status) = child.try_wait().map_err(io)? {
            return Err(io(format!(
                "guest exited before complete product evidence: {status}"
            )));
        }
        std::thread::sleep(Duration::from_millis(100));
    })();
    let _ = child.kill();
    let _ = child.wait();
    if report::sha256_file(&firmware)? != firmware_hash {
        return Err(io("firmware ROM changed during boot"));
    }
    result
}
fn validate_mapping(proof: &Value) -> Result<(), ConduitosError> {
    let evidence = &proof["mapping"];
    let m = &evidence["mapping"];
    let f = &proof["face"]["framebuffer"];
    let bytes = f["pitch"]
        .as_u64()
        .and_then(|pitch| {
            f["height"]
                .as_u64()
                .and_then(|height| pitch.checked_mul(height))
        })
        .ok_or_else(|| io("framebuffer extent missing"))?;
    let pages = bytes.div_ceil(4096);
    let descriptors = m["descriptors_read"].as_u64().unwrap_or(0);
    if evidence["schema"] != "conduit.conduitos/loongarch64-framebuffer-mapping@1"
        || evidence["cache_policy"] != "mat-observed-qemu-tcg-shared-ram"
        || evidence["store_completion"] != "volatile-stores-dbar0"
        || evidence["exclusive_writer"] != true
        || evidence["framebuffer_reserved"] != true
        || evidence["table_page_custody"] != "bootloader-reclaimable"
        || evidence["physical_memory_type_claimed"] != false
        || bytes == 0
        || bytes > 8 * 1024 * 1024
        || m["byte_length"] != bytes
        || m["checked_pages"] != pages
        || !(pages..=pages * 4).contains(&descriptors)
        || m["memory_access_type"].as_u64().is_none_or(|v| v > 2)
        || m["regime"]["virtual_bits"] != 48
        || m["regime"]["reduced_address_config"] != 0
        || m["regime"]["crmd"]
            .as_u64()
            .is_none_or(|v| v & 0x1fb != 0xb0)
        || m["regime"]["pwcl"] != 333763884_u64
        || m["regime"]["pwch"] != 615_u64
        || m["regime"]["stlbps"] != 12
        || m["regime"]["pgdh"]
            .as_u64()
            .is_none_or(|v| v == 0 || !v.is_multiple_of(4096))
        || m["physical_start"].as_u64().is_none_or(|v| {
            v == 0
                || !v.is_multiple_of(4096)
                || v.checked_add(bytes).is_none_or(|end| end > 1 << 48)
        })
        || m["virtual_start"]
            .as_u64()
            .is_none_or(|v| !v.is_multiple_of(4096))
    {
        return Err(io(
            "mapping or completion evidence disagrees with admitted scanout",
        ));
    }
    Ok(())
}
/// The graphical proof must refer to the same finite Base and boot inventory,
/// not merely a screenshot with matching dimensions.
pub(super) fn validate_snapshot(snapshot: &Value, product: &Value) -> Result<(), ConduitosError> {
    let frame = &product["graphical"]["face"]["framebuffer"];
    let bases = snapshot["bases"]
        .as_array()
        .ok_or_else(|| io("Base inventory missing"))?;
    if bases.len() != 8 {
        return Err(io(
            "graphical product requires seven ordinary Bases plus scanout",
        ));
    }
    let selected = bases
        .iter()
        .filter(|base| base["base_id"] == frame["base_id"])
        .collect::<Vec<_>>();
    if selected.len() != 1
        || selected[0]["kind_id"] != "conduitos.base/framebuffer@1"
        || selected[0]["host_id"] != product["host_id"]
        || selected[0]["boot_id"] != product["boot_id"]
        || selected[0]["state"] != "Available"
        || selected[0]["capacity_units"]
            != product["graphical"]["mapping"]["mapping"]["byte_length"]
    {
        return Err(io(
            "captured scanout Base differs from current Host/Boot inventory",
        ));
    }
    let sealed = snapshot["sealed_boot_provenance"]
        .as_array()
        .ok_or_else(|| io("boot provenance missing"))?;
    let records = sealed
        .iter()
        .filter(|entry| {
            entry["host_id"] == product["host_id"] && entry["boot_id"] == product["boot_id"]
        })
        .collect::<Vec<_>>();
    if records.len() != 1 {
        return Err(io("exact current boot provenance required"));
    }
    let record = records[0];
    let frames = record["framebuffers"]
        .as_array()
        .ok_or_else(|| io("boot framebuffer observation missing"))?;
    if record["build_id"] != product["build_id"]
        || record["image_id"] != product["image_id"]
        || record["firmware_environment"] != "uefi64"
        || frames.len() != 1
        || frames[0]["base_id"] != frame["base_id"]
        || frames[0]["width"] != frame["width"]
        || frames[0]["height"] != frame["height"]
        || frames[0]["pitch_bytes"] != frame["pitch"]
        || frames[0]["bits_per_pixel"] != frame["bits_per_pixel"]
    {
        return Err(io(
            "captured framebuffer differs from sealed boot observation",
        ));
    }
    Ok(())
}

fn io(error: impl std::fmt::Display) -> ConduitosError {
    refusal("loongarch64-graphical-proof-unavailable", error.to_string())
}
